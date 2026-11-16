//! Outils partagés par les tests du binaire.
#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const BINAIRE: &str = env!("CARGO_BIN_EXE_paravent");

pub const DECLARATION: &str = r#"{
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

/// Un dossier de travail jetable, retiré en fin de test.
pub struct Dossier(pub PathBuf);

impl Dossier {
    pub fn new() -> Self {
        static NUMERO: AtomicUsize = AtomicUsize::new(0);
        let chemin = std::env::temp_dir().join(format!(
            "paravent-test-{}-{}",
            std::process::id(),
            NUMERO.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&chemin).unwrap();
        Self(chemin)
    }

    /// Un dossier muni d'une déclaration et d'une clé réservée à son propriétaire.
    pub fn equipe() -> Self {
        let dossier = Self::new();
        dossier.ecrire("declaration.json", DECLARATION);
        dossier.ecrire_cle("cle", 0o600);
        dossier
    }

    pub fn chemin(&self, nom: &str) -> PathBuf {
        self.0.join(nom)
    }

    pub fn ecrire(&self, nom: &str, contenu: &str) -> PathBuf {
        let chemin = self.chemin(nom);
        std::fs::write(&chemin, contenu).unwrap();
        chemin
    }

    pub fn ecrire_cle(&self, nom: &str, mode: u32) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let chemin = self.chemin(nom);
        std::fs::write(&chemin, [42u8; 32]).unwrap();
        std::fs::set_permissions(&chemin, std::fs::Permissions::from_mode(mode)).unwrap();
        chemin
    }
}

impl Drop for Dossier {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct Resultat {
    pub code: i32,
    pub sortie: String,
    pub erreur: String,
}

impl From<Output> for Resultat {
    fn from(o: Output) -> Self {
        Self {
            code: o.status.code().unwrap_or(-1),
            sortie: String::from_utf8_lossy(&o.stdout).into_owned(),
            erreur: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }
}

/// Lance le binaire dans `dossier`, avec `entree` sur son entrée standard.
pub fn paravent(dossier: &Path, arguments: &[&str], entree: &[u8]) -> Resultat {
    let mut enfant = Command::new(BINAIRE)
        .args(arguments)
        .current_dir(dossier)
        .env_remove("PARAVENT_CLE")
        .env("PARAVENT", BINAIRE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    enfant.stdin.take().unwrap().write_all(entree).unwrap();
    enfant.wait_with_output().unwrap().into()
}
