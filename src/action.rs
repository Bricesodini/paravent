//! Actions en attente (garantie 5) : rien d'individuel ne part sans un humain.
//!
//! L'agent prépare une action : une cible (une référence), un modèle à trous contrôlé, son nom
//! d'auteur. La source la garde. Un humain la valide ou la refuse là où il voit les données en
//! clair. La source ne l'exécute qu'après validation. Contrat : `docs/adr/0002-actions-en-attente.md`.
//!
//! Une action ne contient aucune valeur personnelle : le modèle n'est rempli qu'à l'exécution.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::modele::ModeleControle;
use crate::reference::Reference;

/// Pourquoi une action est refusée ou ne change pas d'état.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurAction {
    /// Identifiant d'action, nature, auteur ou humain mal formé.
    IdentifiantInvalide,
    /// La date n'est pas au format `AAAA-MM-JJThh:mm:ss` suivi de `Z` ou d'un décalage.
    DateInvalide,
    /// Le modèle a été contrôlé pour un autre type que celui de la cible.
    TypeDifferent,
    /// L'état de l'action ne permet pas ce passage.
    EtatIncompatible,
    /// L'auteur d'une action ne peut pas la décider lui-même.
    AuteurDecideur,
    /// L'action lue est incohérente : état sans la décision ou la date qu'il exige.
    Incoherente,
}

impl fmt::Display for ErreurAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::IdentifiantInvalide => {
                "identifiant invalide : 1 à 64 lettres, chiffres, « _ », « - », « . » ou « @ »"
            }
            Self::DateInvalide => {
                "date invalide : attendu AAAA-MM-JJThh:mm:ss puis Z ou un décalage"
            }
            Self::TypeDifferent => {
                "le modèle a été contrôlé pour un autre type que celui de la cible"
            }
            Self::EtatIncompatible => "l'état de l'action ne permet pas ce passage",
            Self::AuteurDecideur => "l'auteur d'une action ne peut pas la décider lui-même",
            Self::Incoherente => {
                "action incohérente : son état exige une décision ou une date absente"
            }
        })
    }
}

impl std::error::Error for ErreurAction {}

fn identifiant(texte: &str) -> Result<String, ErreurAction> {
    let valide = (1..=64).contains(&texte.chars().count())
        && texte
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'));
    if valide {
        Ok(texte.to_string())
    } else {
        Err(ErreurAction::IdentifiantInvalide)
    }
}

/// Une date et une heure, données par la source : `2030-01-15T09:30:00Z`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Horodatage(String);

impl Horodatage {
    pub fn new(texte: &str) -> Result<Self, ErreurAction> {
        let o = texte.as_bytes();
        let chiffres = |plage: std::ops::Range<usize>| {
            o.get(plage)
                .is_some_and(|p| p.iter().all(u8::is_ascii_digit))
        };
        let debut = chiffres(0..4)
            && o.get(4) == Some(&b'-')
            && chiffres(5..7)
            && o.get(7) == Some(&b'-')
            && chiffres(8..10)
            && o.get(10) == Some(&b'T')
            && chiffres(11..13)
            && o.get(13) == Some(&b':')
            && chiffres(14..16)
            && o.get(16) == Some(&b':')
            && chiffres(17..19);
        if !debut {
            return Err(ErreurAction::DateInvalide);
        }
        let mut reste = &o[19..];
        if let Some((b'.', fraction)) = reste.split_first() {
            let n = fraction.iter().take_while(|c| c.is_ascii_digit()).count();
            if n == 0 {
                return Err(ErreurAction::DateInvalide);
            }
            reste = &fraction[n..];
        }
        let fuseau = match reste {
            b"Z" => true,
            [b'+' | b'-', h1, h2, b':', m1, m2] => {
                [h1, h2, m1, m2].iter().all(|c| c.is_ascii_digit())
            }
            _ => false,
        };
        if !fuseau {
            return Err(ErreurAction::DateInvalide);
        }
        Ok(Self(texte.to_string()))
    }

    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Horodatage {
    type Error = ErreurAction;
    fn try_from(texte: String) -> Result<Self, ErreurAction> {
        Self::new(&texte)
    }
}

impl From<Horodatage> for String {
    fn from(h: Horodatage) -> String {
        h.0
    }
}

/// Un humain qui décide. La source n'en construit un que depuis une session où une personne
/// s'est authentifiée : c'est elle qui l'atteste, la bibliothèque ne peut pas le vérifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Humain(String);

impl Humain {
    /// À n'appeler que pour une personne authentifiée, jamais pour un agent.
    pub fn atteste(identifiant_humain: &str) -> Result<Self, ErreurAction> {
        identifiant(identifiant_humain).map(Self)
    }
}

/// L'état d'une action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Etat {
    EnAttente,
    Validee,
    Refusee,
    Executee,
}

impl Etat {
    pub fn libelle(self) -> &'static str {
        match self {
            Self::EnAttente => "en_attente",
            Self::Validee => "validee",
            Self::Refusee => "refusee",
            Self::Executee => "executee",
        }
    }
}

/// La décision d'un humain : qui, quand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    par: String,
    le: Horodatage,
}

impl Decision {
    pub fn par(&self) -> &str {
        &self.par
    }

    pub fn le(&self) -> &Horodatage {
        &self.le
    }
}

/// Une action préparée par un agent, que seul un humain peut déclencher.
///
/// Ses champs ne se modifient que par [`Action::valider`], [`Action::refuser`] et
/// [`Action::marquer_executee`]. La relecture depuis du JSON refuse une action incohérente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ActionBrute", into = "ActionBrute")]
pub struct Action {
    id: String,
    nature: String,
    cible: Reference,
    modele: String,
    champs: Vec<String>,
    auteur: String,
    creee_le: Horodatage,
    etat: Etat,
    decision: Option<Decision>,
    executee_le: Option<Horodatage>,
}

impl Action {
    /// Prépare une action, en attente.
    ///
    /// `nature` dit ce que la source fera (`courriel`, `rappel`…) ; `auteur` nomme l'agent.
    pub fn proposer(
        id: &str,
        nature: &str,
        cible: Reference,
        modele: &ModeleControle,
        auteur: &str,
        le: Horodatage,
    ) -> Result<Self, ErreurAction> {
        if cible.type_objet() != modele.type_objet() {
            return Err(ErreurAction::TypeDifferent);
        }
        Ok(Self {
            id: identifiant(id)?,
            nature: identifiant(nature)?,
            cible,
            modele: modele.texte().to_string(),
            champs: modele.champs().to_vec(),
            auteur: identifiant(auteur)?,
            creee_le: le,
            etat: Etat::EnAttente,
            decision: None,
            executee_le: None,
        })
    }

    /// Un humain valide l'action.
    pub fn valider(&mut self, par: Humain, le: Horodatage) -> Result<(), ErreurAction> {
        self.decider(Etat::Validee, par, le)
    }

    /// Un humain refuse l'action.
    pub fn refuser(&mut self, par: Humain, le: Horodatage) -> Result<(), ErreurAction> {
        self.decider(Etat::Refusee, par, le)
    }

    fn decider(&mut self, etat: Etat, par: Humain, le: Horodatage) -> Result<(), ErreurAction> {
        if self.etat != Etat::EnAttente {
            return Err(ErreurAction::EtatIncompatible);
        }
        if par.0 == self.auteur {
            return Err(ErreurAction::AuteurDecideur);
        }
        self.decision = Some(Decision { par: par.0, le });
        self.etat = etat;
        Ok(())
    }

    /// La source a exécuté l'action. Seule une action validée y passe : c'est le seul chemin
    /// vers l'état « exécutée », et il exige la décision datée d'un humain.
    pub fn marquer_executee(&mut self, le: Horodatage) -> Result<(), ErreurAction> {
        if self.etat != Etat::Validee || self.decision.is_none() {
            return Err(ErreurAction::EtatIncompatible);
        }
        self.executee_le = Some(le);
        self.etat = Etat::Executee;
        Ok(())
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn nature(&self) -> &str {
        &self.nature
    }

    pub fn cible(&self) -> &Reference {
        &self.cible
    }

    /// Le texte du modèle, non rempli.
    pub fn modele(&self) -> &str {
        &self.modele
    }

    pub fn champs(&self) -> &[String] {
        &self.champs
    }

    pub fn auteur(&self) -> &str {
        &self.auteur
    }

    pub fn etat(&self) -> Etat {
        self.etat
    }

    pub fn decision(&self) -> Option<&Decision> {
        self.decision.as_ref()
    }

    /// Ce que l'agent peut lire d'une action : ni le nom de qui a décidé, ni aucune valeur.
    pub fn resume(&self) -> String {
        format!(
            "action={} nature={} cible={} etat={} champs={}",
            self.id,
            self.nature,
            self.cible,
            self.etat.libelle(),
            self.champs.join(",")
        )
    }
}

/// La forme sérialisée d'une action.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionBrute {
    id: String,
    nature: String,
    cible: String,
    modele: String,
    champs: Vec<String>,
    auteur: String,
    creee_le: Horodatage,
    etat: Etat,
    decision: Option<Decision>,
    executee_le: Option<Horodatage>,
}

impl From<Action> for ActionBrute {
    fn from(a: Action) -> Self {
        Self {
            id: a.id,
            nature: a.nature,
            cible: a.cible.to_string(),
            modele: a.modele,
            champs: a.champs,
            auteur: a.auteur,
            creee_le: a.creee_le,
            etat: a.etat,
            decision: a.decision,
            executee_le: a.executee_le,
        }
    }
}

impl TryFrom<ActionBrute> for Action {
    type Error = ErreurAction;

    fn try_from(b: ActionBrute) -> Result<Self, ErreurAction> {
        let cible = Reference::from_str(&b.cible).map_err(|_| ErreurAction::Incoherente)?;
        let decidee = b.decision.is_some();
        let coherente = match b.etat {
            Etat::EnAttente => !decidee && b.executee_le.is_none(),
            Etat::Validee | Etat::Refusee => decidee && b.executee_le.is_none(),
            Etat::Executee => decidee && b.executee_le.is_some(),
        };
        if !coherente {
            return Err(ErreurAction::Incoherente);
        }
        if let Some(decision) = &b.decision {
            identifiant(&decision.par)?;
            if decision.par == b.auteur {
                return Err(ErreurAction::AuteurDecideur);
            }
        }
        Ok(Self {
            id: identifiant(&b.id)?,
            nature: identifiant(&b.nature)?,
            cible,
            modele: b.modele,
            champs: b.champs,
            auteur: identifiant(&b.auteur)?,
            creee_le: b.creee_le,
            etat: b.etat,
            decision: b.decision,
            executee_le: b.executee_le,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declaration::Declaration;
    use crate::modele::Modele;

    fn date(texte: &str) -> Horodatage {
        Horodatage::new(texte).unwrap()
    }

    fn modele(type_objet: &str) -> ModeleControle {
        let d = Declaration::lire(
            r#"{"version": 1, "types": {
                "contact": {"publics": ["rôle"], "personnels": ["prénom"]},
                "classe": {"publics": ["niveau"]}
            }}"#,
        )
        .unwrap();
        Modele::analyser("Bonjour {prénom}")
            .unwrap()
            .controler(&d, "contact")
            .map(|c| {
                if type_objet == "contact" {
                    c
                } else {
                    Modele::analyser("Niveau {niveau}")
                        .unwrap()
                        .controler(&d, type_objet)
                        .unwrap()
                }
            })
            .unwrap()
    }

    fn action() -> Action {
        Action::proposer(
            "a-17",
            "courriel",
            Reference::fournie("contact", "contact-k3v9x2").unwrap(),
            &modele("contact"),
            "agent-recette",
            date("2030-01-15T09:30:00Z"),
        )
        .unwrap()
    }

    fn humain() -> Humain {
        Humain::atteste("camille.roche").unwrap()
    }

    #[test]
    fn une_action_proposee_est_en_attente_et_sans_valeur() {
        let a = action();
        assert_eq!(a.etat(), Etat::EnAttente);
        assert_eq!(a.champs(), ["prénom"]);
        assert_eq!(a.modele(), "Bonjour {prénom}");
        assert!(a.decision().is_none());
        assert_eq!(
            a.resume(),
            "action=a-17 nature=courriel cible=contact-k3v9x2 etat=en_attente champs=prénom"
        );
    }

    #[test]
    fn rien_ne_s_execute_sans_validation() {
        let mut a = action();
        assert_eq!(
            a.marquer_executee(date("2030-01-15T10:00:00Z")),
            Err(ErreurAction::EtatIncompatible)
        );
        a.refuser(humain(), date("2030-01-15T09:45:00+01:00"))
            .unwrap();
        assert_eq!(a.etat(), Etat::Refusee);
        assert_eq!(
            a.marquer_executee(date("2030-01-15T10:00:00Z")),
            Err(ErreurAction::EtatIncompatible)
        );
    }

    #[test]
    fn une_action_validee_s_execute_une_fois() {
        let mut a = action();
        a.valider(humain(), date("2030-01-15T09:45:00Z")).unwrap();
        assert_eq!(a.etat(), Etat::Validee);
        assert_eq!(a.decision().unwrap().par(), "camille.roche");
        assert_eq!(a.decision().unwrap().le().texte(), "2030-01-15T09:45:00Z");
        a.marquer_executee(date("2030-01-15T10:00:00.250Z"))
            .unwrap();
        assert_eq!(a.etat(), Etat::Executee);
        assert_eq!(
            a.marquer_executee(date("2030-01-15T10:01:00Z")),
            Err(ErreurAction::EtatIncompatible)
        );
        assert_eq!(
            a.valider(humain(), date("2030-01-15T10:02:00Z")),
            Err(ErreurAction::EtatIncompatible)
        );
        // Le résumé pour l'agent ne nomme pas qui a décidé.
        assert!(!a.resume().contains("camille"));
    }

    #[test]
    fn l_auteur_ne_decide_pas_lui_meme() {
        let mut a = action();
        let auteur = Humain::atteste("agent-recette").unwrap();
        assert_eq!(
            a.valider(auteur, date("2030-01-15T09:45:00Z")),
            Err(ErreurAction::AuteurDecideur)
        );
        assert_eq!(a.etat(), Etat::EnAttente);
    }

    #[test]
    fn la_cible_et_le_modele_sont_du_meme_type() {
        let e = Action::proposer(
            "a-18",
            "courriel",
            Reference::fournie("contact", "contact-k3v9x2").unwrap(),
            &modele("classe"),
            "agent-recette",
            date("2030-01-15T09:30:00Z"),
        )
        .unwrap_err();
        assert_eq!(e, ErreurAction::TypeDifferent);
    }

    #[test]
    fn refuse_les_identifiants_et_les_dates_mal_formes() {
        for texte in ["", "a b", "a/b", &"x".repeat(65)] {
            assert_eq!(
                Humain::atteste(texte).unwrap_err(),
                ErreurAction::IdentifiantInvalide,
                "« {texte} »"
            );
        }
        for texte in [
            "",
            "2030-01-15",
            "2030-01-15 09:30:00Z",
            "2030-01-15T09:30:00",
            "2030-01-15T09:30:00.Z",
            "2030-01-15T09:30:00+0100",
            "le 15 janvier",
        ] {
            assert_eq!(
                Horodatage::new(texte).unwrap_err(),
                ErreurAction::DateInvalide,
                "« {texte} »"
            );
        }
    }

    #[test]
    fn se_serialise_et_se_relit() {
        let mut a = action();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(
            json,
            concat!(
                r#"{"id":"a-17","nature":"courriel","cible":"contact-k3v9x2","#,
                r#""modele":"Bonjour {prénom}","champs":["prénom"],"auteur":"agent-recette","#,
                r#""creee_le":"2030-01-15T09:30:00Z","etat":"en_attente","#,
                r#""decision":null,"executee_le":null}"#
            )
        );
        assert_eq!(serde_json::from_str::<Action>(&json).unwrap(), a);

        a.valider(humain(), date("2030-01-15T09:45:00Z")).unwrap();
        a.marquer_executee(date("2030-01-15T10:00:00Z")).unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""etat":"executee""#));
        assert!(json.contains(r#""decision":{"par":"camille.roche","le":"2030-01-15T09:45:00Z"}"#));
        assert_eq!(serde_json::from_str::<Action>(&json).unwrap(), a);
    }

    /// Le JSON ne permet pas de fabriquer une action exécutée ou validée sans décision.
    #[test]
    fn la_relecture_refuse_une_action_incoherente() {
        let base = serde_json::to_value(action()).unwrap();
        let avec = |cles: &[(&str, serde_json::Value)]| {
            let mut v = base.clone();
            for (cle, valeur) in cles {
                v[*cle] = valeur.clone();
            }
            serde_json::from_value::<Action>(v).is_ok()
        };
        let decision = serde_json::json!({"par": "camille.roche", "le": "2030-01-15T09:45:00Z"});
        let date = serde_json::json!("2030-01-15T10:00:00Z");

        assert!(avec(&[]));
        assert!(!avec(&[("etat", "executee".into())]));
        assert!(!avec(&[("etat", "validee".into())]));
        assert!(!avec(&[
            ("etat", "executee".into()),
            ("executee_le", date.clone())
        ]));
        assert!(!avec(&[
            ("etat", "executee".into()),
            ("decision", decision.clone())
        ]));
        assert!(!avec(&[("decision", decision.clone())]));
        assert!(!avec(&[("etat", "en_cours".into())]));
        assert!(!avec(&[("cible", "1042".into())]));
        assert!(!avec(&[("surprise", true.into())]));
        assert!(!avec(&[
            ("etat", "validee".into()),
            (
                "decision",
                serde_json::json!({"par": "agent-recette", "le": "2030-01-15T09:45:00Z"})
            ),
        ]));
        assert!(!avec(&[
            ("etat", "validee".into()),
            (
                "decision",
                serde_json::json!({"par": "camille.roche", "le": "hier"})
            ),
        ]));
        assert!(avec(&[
            ("etat", "validee".into()),
            ("decision", decision.clone())
        ]));
        assert!(avec(&[
            ("etat", "executee".into()),
            ("decision", decision),
            ("executee_le", date),
        ]));
    }
}
