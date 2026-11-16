//! L'outil Python d'exemple passe le banc anti-fuite ; le même outil, sans l'adaptateur, échoue.
//! Demande `python3` sur le chemin.
#![cfg(unix)]

mod commun;

use std::path::Path;
use std::process::Command;

use commun::{Dossier, BINAIRE};

/// Copie l'outil d'exemple et l'adaptateur dans un dossier jetable, puis joue son plan.
fn banc_de_l_exemple(preparer: impl Fn(&Dossier)) -> (i32, String) {
    let depot = Path::new(env!("CARGO_MANIFEST_DIR"));
    let d = Dossier::new();
    let cle = d.ecrire_cle("cle", 0o600);
    for nom in ["outil.py", "declaration.json", "donnees.jsonl", "plan.json"] {
        std::fs::copy(depot.join("exemples/outil_python").join(nom), d.chemin(nom)).unwrap();
    }
    std::fs::copy(depot.join("python/paravent.py"), d.chemin("paravent.py")).unwrap();
    preparer(&d);
    let r = Command::new(BINAIRE)
        .args(["banc", "--plan", "plan.json"])
        .current_dir(&d.0)
        .env("PARAVENT_BIN", BINAIRE)
        .env("PARAVENT_CLE", cle)
        .output()
        .unwrap();
    let sortie = String::from_utf8_lossy(&r.stdout).into_owned();
    assert!(!sortie.contains("PIEGE-"), "le banc a réaffiché un piège");
    (r.status.code().unwrap_or(-1), sortie)
}

#[test]
fn l_outil_python_d_exemple_passe_le_banc() {
    let (code, sortie) = banc_de_l_exemple(|_| {});
    assert_eq!(code, 0, "{sortie}");
    assert_eq!(sortie, "BANC_OK commandes=6 pieges=4\n");
}

#[test]
fn le_meme_outil_sans_l_adaptateur_echoue() {
    // Un faux adaptateur qui écrit tout en clair, comme le ferait un outil non branché.
    let naif = r#"
import json, sys
class Sortie:
    def __init__(self, declaration, **_): pass
    def __enter__(self): return self
    def __exit__(self, *_): return False
    def emettre(self, type_objet, identifiant, champs=None):
        print(json.dumps({"type": type_objet, "id": identifiant, "champs": champs}))
    def etat(self, code, **comptes):
        print(code, comptes)
"#;
    let (code, sortie) = banc_de_l_exemple(|d| {
        d.ecrire("paravent.py", naif);
    });
    assert_eq!(code, 10, "{sortie}");
    // La liste sort en clair (accents échappés par json.dumps) ; la panne sort sa trace brute.
    assert!(sortie.contains("FUITE commande=1 emplacement=stdout piege=1 variante=clair"));
    assert!(sortie.contains("FUITE commande=1 emplacement=stdout piege=2 variante=json"));
    assert!(sortie.contains("FUITE commande=4 emplacement=stderr piege=3 variante=clair"));
    assert!(!sortie.contains("commande=3 "), "{sortie}");
}
