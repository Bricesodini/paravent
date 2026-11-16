//! Le filtre : d'un objet en clair à sa forme sûre (garantie 2 et format d'échange).
//!
//! Entrée, un objet JSON par ligne :
//!
//! ```json
//! {"type": "contact", "id": "1042", "champs": {"prénom": "…", "rôle": "enseignante"}}
//! ```
//!
//! Une source qui attribue ses propres références envoie `"ref"` à la place de `"id"`.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value};

use crate::declaration::{Classe, Declaration};
use crate::reference::{ErreurReference, Fabrique, Reference};

/// Pourquoi un objet est refusé. Aucun message ne recopie la ligne reçue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurFiltre {
    /// La ligne n'est pas un objet au format d'échange.
    LigneIllisible,
    /// Le type de l'objet n'est pas déclaré.
    TypeInconnu,
    /// L'outil a envoyé un champ secret, qu'il n'aurait pas dû lire.
    ChampSecret,
    /// Deux objets du même type donnent la même référence.
    Collision,
    /// La référence fournie par la source est mal formée.
    ReferenceInvalide,
}

impl ErreurFiltre {
    /// Le code stable de l'erreur, pour les agents et les scripts.
    pub fn code(self) -> &'static str {
        match self {
            Self::LigneIllisible => "LIGNE_ILLISIBLE",
            Self::TypeInconnu => "TYPE_INCONNU",
            Self::ChampSecret => "CHAMP_SECRET",
            Self::Collision => "COLLISION",
            Self::ReferenceInvalide => "REFERENCE_INVALIDE",
        }
    }
}

impl fmt::Display for ErreurFiltre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LigneIllisible => "la ligne n'est pas un objet au format d'échange",
            Self::TypeInconnu => "le type de l'objet n'est pas déclaré",
            Self::ChampSecret => "l'outil a envoyé un champ secret",
            Self::Collision => "deux objets du même type donnent la même référence",
            Self::ReferenceInvalide => "la référence fournie par la source est mal formée",
        })
    }
}

impl std::error::Error for ErreurFiltre {}

/// Ce que l'agent apprend d'un champ personnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Renseigne,
    Vide,
}

impl Presence {
    fn de(valeur: &Value) -> Self {
        let vide = match valeur {
            Value::Null => true,
            Value::String(s) => s.trim().is_empty(),
            Value::Array(a) => a.is_empty(),
            Value::Object(o) => o.is_empty(),
            Value::Bool(_) | Value::Number(_) => false,
        };
        if vide {
            Self::Vide
        } else {
            Self::Renseigne
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Self::Renseigne => "renseigné",
            Self::Vide => "vide",
        }
    }
}

/// La forme sûre d'un objet : rien d'autre ne sort du filtre.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjetSur {
    pub reference: Reference,
    pub publics: BTreeMap<String, Value>,
    pub personnels: BTreeMap<String, Presence>,
}

impl ObjetSur {
    /// Une ligne lisible : la référence, les champs publics, la présence des champs personnels.
    pub fn en_texte(&self) -> String {
        let mut parties = vec![self.reference.to_string()];
        for (champ, valeur) in &self.publics {
            parties.push(format!("{champ}={valeur}"));
        }
        for (champ, presence) in &self.personnels {
            parties.push(format!("{champ}:{}", presence.libelle()));
        }
        parties.join("\t")
    }

    /// La même chose en JSON.
    pub fn en_json(&self) -> Value {
        let personnels: Map<String, Value> = self
            .personnels
            .iter()
            .map(|(champ, p)| (champ.clone(), Value::from(p.libelle())))
            .collect();
        let publics: Map<String, Value> = self
            .publics
            .iter()
            .map(|(champ, v)| (champ.clone(), v.clone()))
            .collect();
        let mut objet = Map::new();
        objet.insert("ref".into(), Value::from(self.reference.to_string()));
        objet.insert("type".into(), Value::from(self.reference.type_objet()));
        objet.insert("publics".into(), Value::Object(publics));
        objet.insert("personnels".into(), Value::Object(personnels));
        Value::Object(objet)
    }
}

/// Les comptes d'un type : le total, puis les valeurs des champs publics déclarés à compter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompteType {
    pub total: u64,
    pub par_champ: BTreeMap<String, BTreeMap<String, u64>>,
}

/// Les comptes de tout ce que le filtre a laissé passer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comptes(pub BTreeMap<String, CompteType>);

impl Comptes {
    pub fn en_texte(&self) -> String {
        let mut lignes = vec!["# comptes".to_string()];
        for (type_objet, compte) in &self.0 {
            lignes.push(format!("{type_objet}\t{}", compte.total));
            for (champ, valeurs) in &compte.par_champ {
                for (valeur, n) in valeurs {
                    lignes.push(format!("{type_objet}.{champ}={valeur}\t{n}"));
                }
            }
        }
        lignes.join("\n")
    }

    pub fn en_json(&self) -> Value {
        let types: Map<String, Value> = self
            .0
            .iter()
            .map(|(type_objet, compte)| {
                let mut objet = Map::new();
                objet.insert("total".into(), Value::from(compte.total));
                for (champ, valeurs) in &compte.par_champ {
                    let v: Map<String, Value> = valeurs
                        .iter()
                        .map(|(valeur, n)| (valeur.clone(), Value::from(*n)))
                        .collect();
                    objet.insert(champ.clone(), Value::Object(v));
                }
                (type_objet.clone(), Value::Object(objet))
            })
            .collect();
        let mut racine = Map::new();
        racine.insert("comptes".into(), Value::Object(types));
        Value::Object(racine)
    }
}

/// Filtre des objets selon une déclaration et tient les comptes.
#[derive(Debug)]
pub struct Filtre {
    declaration: Declaration,
    fabrique: Fabrique,
    comptes: Comptes,
}

impl Filtre {
    pub fn new(declaration: Declaration, fabrique: Fabrique) -> Self {
        Self {
            declaration,
            fabrique,
            comptes: Comptes::default(),
        }
    }

    /// Filtre une ligne du format d'échange. Une ligne blanche rend `None`.
    pub fn traiter_ligne(&mut self, ligne: &str) -> Result<Option<ObjetSur>, ErreurFiltre> {
        if ligne.trim().is_empty() {
            return Ok(None);
        }
        let valeur: Value =
            serde_json::from_str(ligne).map_err(|_| ErreurFiltre::LigneIllisible)?;
        self.traiter(valeur).map(Some)
    }

    /// Filtre un objet déjà lu.
    pub fn traiter(&mut self, valeur: Value) -> Result<ObjetSur, ErreurFiltre> {
        let Value::Object(mut objet) = valeur else {
            return Err(ErreurFiltre::LigneIllisible);
        };
        let Some(Value::String(type_objet)) = objet.remove("type") else {
            return Err(ErreurFiltre::LigneIllisible);
        };
        let id = objet.remove("id");
        let fournie = objet.remove("ref");
        let champs = match objet.remove("champs") {
            None => Map::new(),
            Some(Value::Object(champs)) => champs,
            Some(_) => return Err(ErreurFiltre::LigneIllisible),
        };
        // Refus par défaut : une clé que le format ne connaît pas n'est pas ignorée.
        if !objet.is_empty() {
            return Err(ErreurFiltre::LigneIllisible);
        }
        if !self.declaration.connait_type(&type_objet) {
            return Err(ErreurFiltre::TypeInconnu);
        }

        // Tout l'objet est contrôlé avant de calculer la référence et de toucher aux comptes.
        let mut publics = BTreeMap::new();
        let mut personnels = BTreeMap::new();
        for (champ, valeur) in champs {
            match self.declaration.classer(&type_objet, &champ) {
                Classe::Public => {
                    publics.insert(champ, valeur);
                }
                Classe::Personnel => {
                    personnels.insert(champ, Presence::de(&valeur));
                }
                Classe::Secret => return Err(ErreurFiltre::ChampSecret),
            }
        }

        let reference = match (id, fournie) {
            (Some(id), None) => {
                let id = match id {
                    Value::String(s) if !s.is_empty() => s,
                    Value::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
                    _ => return Err(ErreurFiltre::LigneIllisible),
                };
                self.fabrique.reference(&type_objet, &id)
            }
            (None, Some(Value::String(texte))) => Reference::fournie(&type_objet, &texte),
            _ => return Err(ErreurFiltre::LigneIllisible),
        }
        .map_err(|e| match e {
            ErreurReference::Collision => ErreurFiltre::Collision,
            ErreurReference::Malformee => ErreurFiltre::ReferenceInvalide,
            ErreurReference::TypeInvalide => ErreurFiltre::TypeInconnu,
        })?;

        let a_compter = self.declaration.comptes(&type_objet);
        let compte = self.comptes.0.entry(type_objet).or_default();
        compte.total += 1;
        for champ in a_compter {
            let valeur = match publics.get(champ) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                _ => continue,
            };
            *compte
                .par_champ
                .entry(champ.clone())
                .or_default()
                .entry(valeur)
                .or_default() += 1;
        }

        Ok(ObjetSur {
            reference,
            publics,
            personnels,
        })
    }

    /// Les comptes des objets passés jusqu'ici.
    pub fn comptes(&self) -> &Comptes {
        &self.comptes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::Cle;

    const DECLARATION: &str = r#"{
      "version": 1,
      "types": {
        "contact": {
          "publics": ["rôle", "état"],
          "personnels": ["prénom", "courriel"],
          "secrets": ["jeton"],
          "comptes": ["état"]
        }
      }
    }"#;

    fn filtre() -> Filtre {
        Filtre::new(
            Declaration::lire(DECLARATION).unwrap(),
            Fabrique::new(Cle::depuis_octets(vec![5; 32]).unwrap()),
        )
    }

    fn passer(f: &mut Filtre, ligne: &str) -> ObjetSur {
        f.traiter_ligne(ligne).unwrap().unwrap()
    }

    #[test]
    fn l_identifiant_devient_une_reference_et_les_valeurs_personnelles_leur_presence() {
        let mut f = filtre();
        let o = passer(
            &mut f,
            r#"{"type":"contact","id":"1042","champs":{"prénom":"Camille","courriel":"","rôle":"enseignante","état":"actif"}}"#,
        );
        assert!(o.reference.to_string().starts_with("contact-"));
        assert_eq!(o.publics["rôle"], "enseignante");
        assert_eq!(o.personnels["prénom"], Presence::Renseigne);
        assert_eq!(o.personnels["courriel"], Presence::Vide);
        for sortie in [o.en_texte(), o.en_json().to_string(), format!("{o:?}")] {
            assert!(!sortie.contains("Camille"), "{sortie}");
            assert!(!sortie.contains("1042"), "{sortie}");
        }
        assert!(o.en_texte().contains("rôle=\"enseignante\""));
        assert!(o.en_texte().contains("prénom:renseigné"));
        assert!(o.en_texte().contains("courriel:vide"));
    }

    #[test]
    fn un_champ_non_declare_ne_sort_que_par_sa_presence() {
        let mut f = filtre();
        let o = passer(
            &mut f,
            r#"{"type":"contact","id":7,"champs":{"téléphone":"06 00 00 00 00"}}"#,
        );
        assert_eq!(o.personnels["téléphone"], Presence::Renseigne);
        assert!(o.publics.is_empty());
        assert!(!o.en_json().to_string().contains("06 00"));
    }

    #[test]
    fn un_champ_secret_recu_est_une_erreur() {
        let mut f = filtre();
        let e = f
            .traiter_ligne(r#"{"type":"contact","id":"1","champs":{"jeton":"VALEUR-TEMOIN"}}"#)
            .unwrap_err();
        assert_eq!(e, ErreurFiltre::ChampSecret);
        assert!(f.comptes().0.is_empty(), "un objet refusé ne se compte pas");
    }

    #[test]
    fn un_type_inconnu_est_refuse() {
        let e = filtre()
            .traiter_ligne(r#"{"type":"élève","id":"1","champs":{}}"#)
            .unwrap_err();
        assert_eq!(e, ErreurFiltre::TypeInconnu);
    }

    #[test]
    fn une_ligne_hors_format_est_refusee() {
        for ligne in [
            "pas du json",
            "[1, 2]",
            r#"{"id":"1"}"#,
            r#"{"type":"contact"}"#,
            r#"{"type":"contact","id":""}"#,
            r#"{"type":"contact","id":1.5}"#,
            r#"{"type":"contact","id":"1","ref":"contact-abcd"}"#,
            r#"{"type":"contact","id":"1","champs":[]}"#,
            r#"{"type":"contact","id":"1","autre":"x"}"#,
        ] {
            assert_eq!(
                filtre().traiter_ligne(ligne).unwrap_err(),
                ErreurFiltre::LigneIllisible,
                "{ligne}"
            );
        }
    }

    #[test]
    fn une_ligne_blanche_est_ignoree() {
        assert_eq!(filtre().traiter_ligne("  ").unwrap(), None);
    }

    #[test]
    fn une_source_peut_fournir_sa_reference() {
        let mut f = filtre();
        let o = passer(&mut f, r#"{"type":"contact","ref":"contact-k3v9x2"}"#);
        assert_eq!(o.reference.to_string(), "contact-k3v9x2");
        assert_eq!(
            f.traiter_ligne(r#"{"type":"contact","ref":"1042"}"#)
                .unwrap_err(),
            ErreurFiltre::ReferenceInvalide
        );
    }

    #[test]
    fn compte_par_type_et_par_etat() {
        let mut f = filtre();
        for (id, etat) in [("1", "actif"), ("2", "actif"), ("3", "parti")] {
            passer(
                &mut f,
                &format!(r#"{{"type":"contact","id":"{id}","champs":{{"état":"{etat}"}}}}"#),
            );
        }
        passer(&mut f, r#"{"type":"contact","id":"4"}"#);
        let comptes = f.comptes();
        assert_eq!(comptes.0["contact"].total, 4);
        assert_eq!(comptes.0["contact"].par_champ["état"]["actif"], 2);
        assert_eq!(
            comptes.en_texte(),
            "# comptes\ncontact\t4\ncontact.état=actif\t2\ncontact.état=parti\t1"
        );
        assert_eq!(
            comptes.en_json().to_string(),
            r#"{"comptes":{"contact":{"total":4,"état":{"actif":2,"parti":1}}}}"#
        );
    }
}
