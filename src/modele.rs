//! Modèles à trous (garantie 4).
//!
//! L'agent écrit un texte dont les valeurs personnelles sont des champs nommés :
//! `Bonjour {prénom}, votre demande est {état}.` Il ne voit jamais le texte rempli : le
//! remplissage se fait du côté de la source, et son résultat est un [`Personnel`].
//!
//! Une accolade littérale se double : `{{` et `}}`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::declaration::{Classe, Declaration};
use crate::personnel::Personnel;

/// Pourquoi un modèle est refusé. Les positions comptent les caractères à partir de 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurModele {
    /// Une accolade ouvrante n'est pas refermée.
    AccoladeNonFermee { position: usize },
    /// Une accolade fermante n'a pas d'ouvrante.
    AccoladeIsolee { position: usize },
    /// Un trou ne nomme aucun champ.
    ChampVide { position: usize },
    /// Le type visé n'est pas déclaré.
    TypeInconnu,
    /// Le champ n'est pas déclaré pour ce type.
    ChampInconnu { champ: String },
    /// Le champ est secret : il ne se lit pas, donc ne s'écrit pas dans un message.
    ChampSecret { champ: String },
    /// Au remplissage, la source n'a pas fourni ce champ.
    ValeurManquante { champ: String },
    /// Au remplissage, ce champ est vide pour cet objet.
    ValeurVide { champ: String },
}

impl fmt::Display for ErreurModele {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccoladeNonFermee { position } => {
                write!(f, "accolade ouvrante non refermée (caractère {position})")
            }
            Self::AccoladeIsolee { position } => write!(
                f,
                "accolade fermante sans ouvrante (caractère {position}) ; une accolade littérale se double"
            ),
            Self::ChampVide { position } => {
                write!(f, "trou sans nom de champ (caractère {position})")
            }
            Self::TypeInconnu => write!(f, "le type visé n'est pas déclaré"),
            Self::ChampInconnu { champ } => {
                write!(f, "le champ « {champ} » n'est pas déclaré pour ce type")
            }
            Self::ChampSecret { champ } => write!(f, "le champ « {champ} » est secret"),
            Self::ValeurManquante { champ } => {
                write!(f, "la source n'a pas fourni le champ « {champ} »")
            }
            Self::ValeurVide { champ } => {
                write!(f, "le champ « {champ} » est vide pour cet objet")
            }
        }
    }
}

impl std::error::Error for ErreurModele {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Texte(String),
    Champ(String),
}

/// Un modèle analysé : du texte et des trous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modele {
    texte: String,
    segments: Vec<Segment>,
}

impl Modele {
    /// Analyse le texte d'un modèle.
    pub fn analyser(texte: &str) -> Result<Self, ErreurModele> {
        let caracteres: Vec<char> = texte.chars().collect();
        let mut segments = Vec::new();
        let mut courant = String::new();
        let mut i = 0;
        while i < caracteres.len() {
            let suivant = caracteres.get(i + 1).copied();
            match caracteres[i] {
                '{' if suivant == Some('{') => {
                    courant.push('{');
                    i += 2;
                }
                '}' if suivant == Some('}') => {
                    courant.push('}');
                    i += 2;
                }
                '}' => return Err(ErreurModele::AccoladeIsolee { position: i + 1 }),
                '{' => {
                    let fin = caracteres[i + 1..]
                        .iter()
                        .position(|c| matches!(c, '{' | '}'))
                        .map(|p| i + 1 + p)
                        .filter(|fin| caracteres[*fin] == '}')
                        .ok_or(ErreurModele::AccoladeNonFermee { position: i + 1 })?;
                    let nom: String = caracteres[i + 1..fin].iter().collect();
                    let nom = nom.trim();
                    if nom.is_empty() {
                        return Err(ErreurModele::ChampVide { position: i + 1 });
                    }
                    if !courant.is_empty() {
                        segments.push(Segment::Texte(std::mem::take(&mut courant)));
                    }
                    segments.push(Segment::Champ(nom.to_string()));
                    i = fin + 1;
                }
                c => {
                    courant.push(c);
                    i += 1;
                }
            }
        }
        if !courant.is_empty() {
            segments.push(Segment::Texte(courant));
        }
        Ok(Self {
            texte: texte.to_string(),
            segments,
        })
    }

    /// Le texte du modèle, tel que l'agent l'a écrit.
    pub fn texte(&self) -> &str {
        &self.texte
    }

    /// Les champs que le modèle utilise, triés, sans doublon.
    pub fn champs(&self) -> Vec<String> {
        let noms: BTreeSet<&String> = self
            .segments
            .iter()
            .filter_map(|s| match s {
                Segment::Champ(nom) => Some(nom),
                Segment::Texte(_) => None,
            })
            .collect();
        noms.into_iter().cloned().collect()
    }

    /// Contrôle le modèle contre la déclaration, pour un type d'objet.
    ///
    /// Ici le refus par défaut joue dans l'autre sens : un champ qui n'est pas déclaré est
    /// refusé, pour qu'une faute de frappe ne parte pas dans un message.
    pub fn controler(
        &self,
        declaration: &Declaration,
        type_objet: &str,
    ) -> Result<ModeleControle, ErreurModele> {
        if !declaration.connait_type(type_objet) {
            return Err(ErreurModele::TypeInconnu);
        }
        let champs = self.champs();
        for champ in &champs {
            match declaration.classe_declaree(type_objet, champ) {
                None => {
                    return Err(ErreurModele::ChampInconnu {
                        champ: champ.clone(),
                    })
                }
                Some(Classe::Secret) => {
                    return Err(ErreurModele::ChampSecret {
                        champ: champ.clone(),
                    })
                }
                Some(Classe::Public | Classe::Personnel) => {}
            }
        }
        Ok(ModeleControle {
            type_objet: type_objet.to_string(),
            modele: self.clone(),
            champs,
        })
    }
}

/// Un modèle contrôlé pour un type : la seule forme qu'une action accepte et qui se remplit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeleControle {
    type_objet: String,
    modele: Modele,
    champs: Vec<String>,
}

impl ModeleControle {
    pub fn type_objet(&self) -> &str {
        &self.type_objet
    }

    pub fn texte(&self) -> &str {
        self.modele.texte()
    }

    /// Les champs utilisés : ce que la source doit fournir au remplissage.
    pub fn champs(&self) -> &[String] {
        &self.champs
    }

    /// Remplit le modèle. À appeler du côté de la source, au moment d'agir.
    ///
    /// Le résultat est personnel : il ne s'affiche pas et ne se journalise pas. Une valeur n'est
    /// jamais relue comme un modèle : des accolades dans une valeur restent des accolades.
    pub fn remplir(
        &self,
        valeurs: &BTreeMap<String, Personnel>,
    ) -> Result<Personnel, ErreurModele> {
        let mut sortie = String::new();
        for segment in &self.modele.segments {
            match segment {
                Segment::Texte(texte) => sortie.push_str(texte),
                Segment::Champ(champ) => {
                    let valeur =
                        valeurs
                            .get(champ)
                            .ok_or_else(|| ErreurModele::ValeurManquante {
                                champ: champ.clone(),
                            })?;
                    if !valeur.est_renseigne() {
                        return Err(ErreurModele::ValeurVide {
                            champ: champ.clone(),
                        });
                    }
                    sortie.push_str(valeur.exposer());
                }
            }
        }
        Ok(Personnel::new(sortie))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLARATION: &str = r#"{
      "version": 1,
      "types": {
        "contact": {
          "publics": ["rôle", "état"],
          "personnels": ["prénom", "courriel"],
          "secrets": ["jeton"]
        }
      }
    }"#;

    fn declaration() -> Declaration {
        Declaration::lire(DECLARATION).unwrap()
    }

    fn valeurs(paires: &[(&str, &str)]) -> BTreeMap<String, Personnel> {
        paires
            .iter()
            .map(|(champ, valeur)| (champ.to_string(), Personnel::new(*valeur)))
            .collect()
    }

    #[test]
    fn rend_les_champs_utilises() {
        let m = Modele::analyser("Bonjour {prénom}, {  rôle }, encore {prénom}.").unwrap();
        assert_eq!(m.champs(), ["prénom", "rôle"]);
        let c = m.controler(&declaration(), "contact").unwrap();
        assert_eq!(c.champs(), ["prénom", "rôle"]);
        assert_eq!(c.type_objet(), "contact");
        assert_eq!(c.texte(), "Bonjour {prénom}, {  rôle }, encore {prénom}.");
    }

    #[test]
    fn refuse_un_champ_inconnu_ou_secret_et_un_type_inconnu() {
        let d = declaration();
        let controler = |texte: &str, type_objet: &str| {
            Modele::analyser(texte)
                .unwrap()
                .controler(&d, type_objet)
                .unwrap_err()
        };
        assert_eq!(
            controler("Bonjour {prenom}", "contact"),
            ErreurModele::ChampInconnu {
                champ: "prenom".into()
            }
        );
        assert_eq!(
            controler("Votre code : {jeton}", "contact"),
            ErreurModele::ChampSecret {
                champ: "jeton".into()
            }
        );
        assert_eq!(
            controler("Bonjour {prénom}", "élève"),
            ErreurModele::TypeInconnu
        );
    }

    #[test]
    fn refuse_une_syntaxe_incorrecte_avec_sa_position() {
        for (texte, attendue) in [
            (
                "Bonjour {prénom",
                ErreurModele::AccoladeNonFermee { position: 9 },
            ),
            ("a {b {c}", ErreurModele::AccoladeNonFermee { position: 3 }),
            (
                "Bonjour prénom}",
                ErreurModele::AccoladeIsolee { position: 15 },
            ),
            ("Bonjour { }", ErreurModele::ChampVide { position: 9 }),
            ("{}", ErreurModele::ChampVide { position: 1 }),
        ] {
            assert_eq!(Modele::analyser(texte).unwrap_err(), attendue, "{texte}");
        }
    }

    #[test]
    fn remplit_du_cote_de_la_source_et_rend_un_texte_personnel() {
        let c = Modele::analyser("Bonjour {prénom}, vous êtes {rôle}. {{sic}}")
            .unwrap()
            .controler(&declaration(), "contact")
            .unwrap();
        let rempli = c
            .remplir(&valeurs(&[("prénom", "Camille"), ("rôle", "enseignante")]))
            .unwrap();
        assert_eq!(
            rempli.exposer(),
            "Bonjour Camille, vous êtes enseignante. {sic}"
        );
        assert!(!format!("{rempli:?}").contains("Camille"));
    }

    #[test]
    fn une_valeur_n_est_jamais_relue_comme_un_modele() {
        let c = Modele::analyser("Bonjour {prénom}")
            .unwrap()
            .controler(&declaration(), "contact")
            .unwrap();
        let rempli = c
            .remplir(&valeurs(&[
                ("prénom", "{courriel}"),
                ("courriel", "x@exemple.invalid"),
            ]))
            .unwrap();
        assert_eq!(rempli.exposer(), "Bonjour {courriel}");
    }

    #[test]
    fn refuse_de_remplir_avec_une_valeur_absente_ou_vide_sans_la_citer() {
        let c = Modele::analyser("Bonjour {prénom}, {courriel}")
            .unwrap()
            .controler(&declaration(), "contact")
            .unwrap();
        let e = c.remplir(&valeurs(&[("prénom", "Camille")])).unwrap_err();
        assert_eq!(
            e,
            ErreurModele::ValeurManquante {
                champ: "courriel".into()
            }
        );
        assert!(!e.to_string().contains("Camille"));
        assert_eq!(
            c.remplir(&valeurs(&[("prénom", "Camille"), ("courriel", "  ")]))
                .unwrap_err(),
            ErreurModele::ValeurVide {
                champ: "courriel".into()
            }
        );
    }

    #[test]
    fn un_modele_sans_trou_est_valide() {
        let c = Modele::analyser("Rappel : la salle ouvre à 9 h.")
            .unwrap()
            .controler(&declaration(), "contact")
            .unwrap();
        assert!(c.champs().is_empty());
        assert_eq!(
            c.remplir(&BTreeMap::new()).unwrap().exposer(),
            "Rappel : la salle ouvre à 9 h."
        );
    }
}
