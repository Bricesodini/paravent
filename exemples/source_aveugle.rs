//! Une source aveugle (niveau 2), tenue en mémoire pour l'exemple.
//!
//!     cargo run --example source_aveugle
//!
//! Tout ce que ce programme affiche est ce qu'un agent verrait : des références, des champs
//! publics, des présences, des états. Les noms et les adresses ne quittent pas la source.

use std::collections::BTreeMap;

use paravent::action::{Action, Etat, Horodatage, Humain};
use paravent::filtre::Filtre;
use paravent::modele::Modele;
use paravent::{Cle, Declaration, Fabrique, Personnel, Reference};
use serde_json::{json, Value};

const DECLARATION: &str = r#"{
  "version": 1,
  "types": {
    "contact": {
      "publics": ["rôle", "état"],
      "personnels": ["prénom", "nom", "courriel"],
      "comptes": ["état"]
    }
  }
}"#;

/// La source : elle détient les données en clair et ne répond qu'en forme sûre.
struct Source {
    declaration: Declaration,
    cle: Cle,
    contacts: Vec<Value>,
    actions: Vec<Action>,
    /// Ce qui est réellement parti : destinataire et texte, tous deux personnels.
    envois: Vec<(Personnel, Personnel)>,
}

impl Source {
    /// Un filtre neuf par réponse : ses comptes ne portent que sur cette réponse.
    fn filtre(&self) -> Filtre {
        Filtre::new(self.declaration.clone(), Fabrique::new(self.cle.clone()))
    }

    /// Répond à l'agent : les objets en forme sûre, puis les comptes.
    fn repondre<'a>(&self, objets: impl Iterator<Item = &'a Value>) -> Result<Vec<String>, String> {
        let mut filtre = self.filtre();
        let mut lignes = Vec::new();
        for objet in objets {
            let sur = filtre.traiter(objet.clone()).map_err(|e| e.to_string())?;
            lignes.push(sur.en_json().to_string());
        }
        lignes.push(filtre.comptes().en_json().to_string());
        Ok(lignes)
    }

    fn lister(&self) -> Result<Vec<String>, String> {
        self.repondre(self.contacts.iter())
    }

    /// Résolution : l'humain a donné un prénom ou un nom, la source rend des références.
    fn resoudre(&self, terme: &str) -> Result<Vec<String>, String> {
        let terme = terme.to_lowercase();
        self.repondre(self.contacts.iter().filter(|c| {
            ["prénom", "nom"].iter().any(|champ| {
                c["champs"][champ]
                    .as_str()
                    .is_some_and(|v| v.to_lowercase().contains(&terme))
            })
        }))
    }

    /// De la référence à l'objet : la source recalcule, l'agent ne peut pas le faire.
    fn trouver(&self, cible: &Reference) -> Option<&Value> {
        let mut fabrique = Fabrique::new(self.cle.clone());
        self.contacts.iter().find(|c| {
            let id = c["id"].as_str().unwrap_or_default();
            fabrique.reference("contact", id).ok().as_ref() == Some(cible)
        })
    }

    /// L'agent dépose une action. Elle attend.
    fn proposer(&mut self, cible: &str, modele: &str, auteur: &str) -> Result<String, String> {
        let cible: Reference = cible.parse().map_err(|_| "référence mal formée")?;
        if self.trouver(&cible).is_none() {
            return Err("référence inconnue".into());
        }
        let controle = Modele::analyser(modele)
            .and_then(|m| m.controler(&self.declaration, cible.type_objet()))
            .map_err(|e| e.to_string())?;
        let id = format!("a-{}", self.actions.len() + 1);
        let action = Action::proposer(&id, "courriel", cible, &controle, auteur, maintenant())
            .map_err(|e| e.to_string())?;
        let resume = action.resume();
        self.actions.push(action);
        Ok(resume)
    }

    /// Appelée par l'écran de validation, depuis la session d'une personne authentifiée.
    fn valider(&mut self, id: &str, humain: Humain) -> Result<(), String> {
        let action = self.actions.iter_mut().find(|a| a.id() == id);
        let action = action.ok_or("action inconnue")?;
        action
            .valider(humain, maintenant())
            .map_err(|e| e.to_string())
    }

    /// Exécute les actions validées : c'est ici, et ici seulement, que le modèle se remplit.
    fn executer(&mut self) -> Result<Vec<String>, String> {
        let mut resumes = Vec::new();
        for i in 0..self.actions.len() {
            if self.actions[i].etat() != Etat::Validee {
                continue;
            }
            let action = &self.actions[i];
            let contact = self.trouver(action.cible()).ok_or("cible disparue")?;
            let lire =
                |champ: &str| Personnel::new(contact["champs"][champ].as_str().unwrap_or(""));
            let valeurs: BTreeMap<String, Personnel> = action
                .champs()
                .iter()
                .map(|champ| (champ.clone(), lire(champ)))
                .collect();
            let texte = Modele::analyser(action.modele())
                .and_then(|m| m.controler(&self.declaration, action.cible().type_objet()))
                .and_then(|m| m.remplir(&valeurs))
                .map_err(|e| e.to_string())?;
            let destinataire = lire("courriel");
            self.envois.push((destinataire, texte));
            self.actions[i]
                .marquer_executee(maintenant())
                .map_err(|e| e.to_string())?;
            resumes.push(self.actions[i].resume());
        }
        Ok(resumes)
    }
}

/// Une vraie source lit l'horloge ; l'exemple reste déterministe.
fn maintenant() -> Horodatage {
    Horodatage::new("2030-01-15T09:30:00Z").expect("date de l'exemple")
}

fn main() -> Result<(), String> {
    let mut source = Source {
        declaration: Declaration::lire(DECLARATION).map_err(|e| e.to_string())?,
        // Une vraie source lit sa clé avec `Cle::lire_fichier`.
        cle: Cle::depuis_octets(vec![7; 32]).map_err(|e| e.to_string())?,
        contacts: vec![
            json!({"type": "contact", "id": "1042", "champs": {"prénom": "Camille", "nom": "Roche",
                   "courriel": "camille.roche@exemple.invalid", "rôle": "enseignante", "état": "actif"}}),
            json!({"type": "contact", "id": "1043", "champs": {"prénom": "Sacha", "nom": "Morel",
                   "courriel": "", "rôle": "direction", "état": "actif"}}),
        ],
        actions: Vec::new(),
        envois: Vec::new(),
    };

    println!("# l'agent liste");
    source.lister()?.iter().for_each(|l| println!("{l}"));

    println!("# l'humain a dit « écris à Camille » : l'agent fait résoudre le prénom");
    let trouves = source.resoudre("camille")?;
    trouves.iter().for_each(|l| println!("{l}"));
    let premier: Value = serde_json::from_str(&trouves[0]).map_err(|e| e.to_string())?;
    let cible = premier["ref"].as_str().unwrap_or_default().to_string();

    println!("# l'agent propose un message, sans connaître ni le prénom ni l'adresse");
    let modele = "Bonjour {prénom}, votre inscription est {état}.";
    println!("{}", source.proposer(&cible, modele, "agent-exemple")?);

    println!("# un modèle fautif est refusé");
    for fautif in ["Bonjour {prenom}", "Bonjour {prénom"] {
        let refus = source
            .proposer(&cible, fautif, "agent-exemple")
            .unwrap_err();
        println!("refus : {refus}");
    }

    println!("# rien ne part tant qu'un humain n'a pas validé");
    println!("exécutées : {}", source.executer()?.len());

    println!("# un humain valide, dans l'application, où il voit le message en clair");
    let humain = Humain::atteste("responsable").map_err(|e| e.to_string())?;
    source.valider("a-1", humain)?;
    source.executer()?.iter().for_each(|l| println!("{l}"));
    println!("envois : {}", source.envois.len());

    // Le texte rempli existe, du côté de la source, et ne s'affiche pas.
    let (destinataire, texte) = &source.envois[0];
    assert_eq!(destinataire.exposer(), "camille.roche@exemple.invalid");
    assert_eq!(
        texte.exposer(),
        "Bonjour Camille, votre inscription est actif."
    );
    println!("message rempli : {texte:?}");
    Ok(())
}
