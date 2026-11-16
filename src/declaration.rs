//! La déclaration : le fichier où un outil classe chaque champ de chaque type d'objet.
//!
//! ```json
//! {
//!   "version": 1,
//!   "types": {
//!     "contact": {
//!       "publics": ["rôle", "état"],
//!       "personnels": ["prénom", "courriel"],
//!       "secrets": ["jeton"],
//!       "comptes": ["état"]
//!     }
//!   }
//! }
//! ```
//!
//! Garantie 1, refus par défaut : un champ ou un type absent est traité comme personnel.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

/// Version du format de déclaration que cette bibliothèque sait lire.
pub const VERSION: u32 = 1;

/// La classe d'un champ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classe {
    /// Peut sortir tel quel.
    Public,
    /// Ne sort jamais ; seule sa présence est connue de l'agent.
    Personnel,
    /// Ne sort jamais et ne doit pas être lu par l'outil.
    Secret,
}

/// Pourquoi une déclaration est refusée. Aucun message ne recopie le contenu du fichier, hors
/// les noms de type et de champ, qui décrivent le schéma et non des personnes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurDeclaration {
    /// Le fichier ne se lit pas.
    Inaccessible,
    /// Le texte n'est pas une déclaration : JSON invalide, clé inconnue ou en double.
    Illisible { ligne: usize, colonne: usize },
    /// La version n'est pas celle que lit cette bibliothèque.
    VersionInconnue { version: u32 },
    /// Un nom de type ne peut pas servir de préfixe de référence.
    NomDeTypeInvalide,
    /// Un champ est classé deux fois dans le même type.
    ChampEnDouble { type_objet: String, champ: String },
    /// Un champ à compter n'est pas public.
    CompteNonPublic { type_objet: String, champ: String },
}

impl fmt::Display for ErreurDeclaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inaccessible => write!(f, "le fichier de déclaration ne se lit pas"),
            Self::Illisible { ligne, colonne } => write!(
                f,
                "déclaration illisible (ligne {ligne}, colonne {colonne}) : JSON invalide, clé inconnue ou en double"
            ),
            Self::VersionInconnue { version } => {
                write!(f, "version de déclaration {version} inconnue (attendue : {VERSION})")
            }
            Self::NomDeTypeInvalide => write!(
                f,
                "nom de type invalide : lettres minuscules, chiffres et « _ » seulement"
            ),
            Self::ChampEnDouble { type_objet, champ } => {
                write!(f, "champ « {champ} » classé deux fois dans le type « {type_objet} »")
            }
            Self::CompteNonPublic { type_objet, champ } => write!(
                f,
                "champ « {champ} » du type « {type_objet} » : seul un champ public se compte"
            ),
        }
    }
}

impl std::error::Error for ErreurDeclaration {}

/// Un nom de type sert de préfixe aux références : lettres minuscules, chiffres et `_`.
pub fn nom_de_type_valide(nom: &str) -> bool {
    !nom.is_empty()
        && nom
            .chars()
            .all(|c| c == '_' || (c.is_alphanumeric() && !c.is_uppercase()))
}

#[derive(Debug, Clone)]
struct TypeDeclare {
    champs: BTreeMap<String, Classe>,
    comptes: Vec<String>,
}

/// Une déclaration lue et vérifiée.
#[derive(Debug, Clone)]
pub struct Declaration {
    types: BTreeMap<String, TypeDeclare>,
}

impl Declaration {
    /// Lit une déclaration depuis son texte JSON.
    pub fn lire(texte: &str) -> Result<Self, ErreurDeclaration> {
        // Le message de serde peut recopier une valeur du fichier : on n'en garde que la position.
        let brut: Brut = serde_json::from_str(texte).map_err(|e| ErreurDeclaration::Illisible {
            ligne: e.line(),
            colonne: e.column(),
        })?;
        if brut.version != VERSION {
            return Err(ErreurDeclaration::VersionInconnue {
                version: brut.version,
            });
        }
        let mut types = BTreeMap::new();
        for (nom, t) in brut.types.0 {
            if !nom_de_type_valide(&nom) {
                return Err(ErreurDeclaration::NomDeTypeInvalide);
            }
            let mut champs = BTreeMap::new();
            let classes = [
                (Classe::Public, &t.publics),
                (Classe::Personnel, &t.personnels),
                (Classe::Secret, &t.secrets),
            ];
            for (classe, liste) in classes {
                for champ in liste {
                    if champs.insert(champ.clone(), classe).is_some() {
                        return Err(ErreurDeclaration::ChampEnDouble {
                            type_objet: nom,
                            champ: champ.clone(),
                        });
                    }
                }
            }
            for champ in &t.comptes {
                if champs.get(champ) != Some(&Classe::Public) {
                    return Err(ErreurDeclaration::CompteNonPublic {
                        type_objet: nom,
                        champ: champ.clone(),
                    });
                }
            }
            types.insert(
                nom,
                TypeDeclare {
                    champs,
                    comptes: t.comptes,
                },
            );
        }
        Ok(Self { types })
    }

    /// Lit une déclaration depuis un fichier.
    pub fn lire_fichier(chemin: &Path) -> Result<Self, ErreurDeclaration> {
        let texte = std::fs::read_to_string(chemin).map_err(|_| ErreurDeclaration::Inaccessible)?;
        Self::lire(&texte)
    }

    /// Vrai si le type est déclaré.
    pub fn connait_type(&self, type_objet: &str) -> bool {
        self.types.contains_key(type_objet)
    }

    /// Classe d'un champ. Refus par défaut : tout ce qui n'est pas déclaré est personnel.
    pub fn classer(&self, type_objet: &str, champ: &str) -> Classe {
        self.types
            .get(type_objet)
            .and_then(|t| t.champs.get(champ))
            .copied()
            .unwrap_or(Classe::Personnel)
    }

    /// Classe d'un champ tel qu'il est déclaré ; `None` s'il ne l'est pas.
    pub fn classe_declaree(&self, type_objet: &str, champ: &str) -> Option<Classe> {
        self.types
            .get(type_objet)
            .and_then(|t| t.champs.get(champ))
            .copied()
    }

    /// Les champs publics dont on compte les valeurs, pour un type.
    pub fn comptes(&self, type_objet: &str) -> &[String] {
        self.types
            .get(type_objet)
            .map(|t| t.comptes.as_slice())
            .unwrap_or(&[])
    }

    /// Les types déclarés, triés.
    pub fn types(&self) -> impl Iterator<Item = &str> {
        self.types.keys().map(String::as_str)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Brut {
    version: u32,
    types: Types,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeBrut {
    #[serde(default)]
    publics: Vec<String>,
    #[serde(default)]
    personnels: Vec<String>,
    #[serde(default)]
    secrets: Vec<String>,
    #[serde(default)]
    comptes: Vec<String>,
}

/// Les types, lus en refusant un nom en double : sinon la seconde définition remplacerait la
/// première en silence, et un champ personnel pourrait devenir public sans qu'on le voie.
struct Types(Vec<(String, TypeBrut)>);

impl<'de> Deserialize<'de> for Types {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Types;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("un objet de types")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut acces: A) -> Result<Types, A::Error> {
                let mut vus: Vec<(String, TypeBrut)> = Vec::new();
                while let Some((nom, t)) = acces.next_entry::<String, TypeBrut>()? {
                    if vus.iter().any(|(n, _)| *n == nom) {
                        return Err(serde::de::Error::custom("type en double"));
                    }
                    vus.push((nom, t));
                }
                Ok(Types(vus))
            }
        }
        d.deserialize_map(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXEMPLE: &str = r#"{
      "version": 1,
      "types": {
        "contact": {
          "publics": ["rôle", "état"],
          "personnels": ["prénom", "courriel"],
          "secrets": ["jeton"],
          "comptes": ["état"]
        },
        "école": { "publics": ["nom", "commune"] }
      }
    }"#;

    #[test]
    fn classe_les_champs_declares() {
        let d = Declaration::lire(EXEMPLE).unwrap();
        assert_eq!(d.classer("contact", "rôle"), Classe::Public);
        assert_eq!(d.classer("contact", "courriel"), Classe::Personnel);
        assert_eq!(d.classer("contact", "jeton"), Classe::Secret);
        assert_eq!(d.comptes("contact"), ["état".to_string()]);
        assert_eq!(d.types().collect::<Vec<_>>(), ["contact", "école"]);
    }

    #[test]
    fn un_champ_absent_est_personnel() {
        let d = Declaration::lire(EXEMPLE).unwrap();
        assert_eq!(d.classer("contact", "téléphone"), Classe::Personnel);
        assert_eq!(d.classer("école", "directrice"), Classe::Personnel);
    }

    #[test]
    fn un_type_absent_est_personnel_en_entier() {
        let d = Declaration::lire(EXEMPLE).unwrap();
        assert!(!d.connait_type("élève"));
        assert_eq!(d.classer("élève", "nom"), Classe::Personnel);
        assert_eq!(d.classer("élève", "rôle"), Classe::Personnel);
        assert!(d.comptes("élève").is_empty());
    }

    #[test]
    fn refuse_un_json_invalide_sans_citer_de_valeur() {
        let texte = r#"{"version": 1, "types": {"contact": {"publics": "VALEUR-TEMOIN"}}}"#;
        let e = Declaration::lire(texte).unwrap_err();
        assert!(matches!(e, ErreurDeclaration::Illisible { ligne: 1, .. }));
        assert!(!e.to_string().contains("VALEUR-TEMOIN"));
        assert!(!format!("{e:?}").contains("VALEUR-TEMOIN"));
    }

    #[test]
    fn refuse_une_cle_inconnue_sans_la_citer() {
        let texte = r#"{"version": 1, "types": {}, "CLE-TEMOIN": true}"#;
        let e = Declaration::lire(texte).unwrap_err();
        assert!(matches!(e, ErreurDeclaration::Illisible { .. }));
        assert!(!e.to_string().contains("CLE-TEMOIN"));

        let texte = r#"{"version": 1, "types": {"contact": {"public": ["rôle"]}}}"#;
        assert!(matches!(
            Declaration::lire(texte),
            Err(ErreurDeclaration::Illisible { .. })
        ));
    }

    #[test]
    fn refuse_une_version_inconnue() {
        let e = Declaration::lire(r#"{"version": 2, "types": {}}"#).unwrap_err();
        assert_eq!(e, ErreurDeclaration::VersionInconnue { version: 2 });
    }

    #[test]
    fn refuse_un_champ_classe_deux_fois() {
        let texte =
            r#"{"version": 1, "types": {"contact": {"publics": ["nom"], "personnels": ["nom"]}}}"#;
        assert_eq!(
            Declaration::lire(texte).unwrap_err(),
            ErreurDeclaration::ChampEnDouble {
                type_objet: "contact".into(),
                champ: "nom".into()
            }
        );
    }

    #[test]
    fn refuse_un_type_defini_deux_fois() {
        let texte = r#"{"version": 1, "types": {
            "contact": {"personnels": ["nom"]},
            "contact": {"publics": ["nom"]}
        }}"#;
        assert!(matches!(
            Declaration::lire(texte),
            Err(ErreurDeclaration::Illisible { .. })
        ));
    }

    #[test]
    fn refuse_un_nom_de_type_qui_ne_peut_pas_prefixer_une_reference() {
        for nom in ["", "Contact", "mon-type", "a b"] {
            let texte = format!(r#"{{"version": 1, "types": {{"{nom}": {{}}}}}}"#);
            assert_eq!(
                Declaration::lire(&texte).unwrap_err(),
                ErreurDeclaration::NomDeTypeInvalide,
                "nom « {nom} »"
            );
        }
    }

    #[test]
    fn ne_compte_que_des_champs_publics() {
        let texte =
            r#"{"version": 1, "types": {"contact": {"personnels": ["nom"], "comptes": ["nom"]}}}"#;
        assert!(matches!(
            Declaration::lire(texte),
            Err(ErreurDeclaration::CompteNonPublic { .. })
        ));
    }

    #[test]
    fn un_fichier_absent_est_une_erreur_sans_chemin() {
        let e = Declaration::lire_fichier(Path::new("/nulle/part/declaration.json")).unwrap_err();
        assert_eq!(e, ErreurDeclaration::Inaccessible);
    }
}
