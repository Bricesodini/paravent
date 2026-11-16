//! Le binaire `paravent modele` : il contrôle, il ne remplit pas.
#![cfg(unix)]

mod commun;

use commun::{paravent, Dossier};

const MODELE: [&str; 5] = [
    "modele",
    "--declaration",
    "declaration.json",
    "--type",
    "contact",
];

#[test]
fn rend_les_champs_utilises() {
    let d = Dossier::equipe();
    let r = paravent(
        &d.0,
        &MODELE,
        "Bonjour {prénom}, vous êtes {rôle}.".as_bytes(),
    );
    assert_eq!(r.code, 0, "{}", r.erreur);
    assert_eq!(r.sortie, "MODELE_OK type=contact champs=prénom,rôle\n");

    let r = paravent(&d.0, &MODELE, "Rappel pour tous.".as_bytes());
    assert_eq!(r.sortie, "MODELE_OK type=contact champs=\n");
}

#[test]
fn refuse_un_champ_inconnu_ou_secret_et_une_syntaxe_incorrecte() {
    let d = Dossier::equipe();
    for modele in [
        "Bonjour {prenom}",
        "Code : {jeton}",
        "Bonjour {prénom",
        "Bonjour }",
    ] {
        let r = paravent(&d.0, &MODELE, modele.as_bytes());
        assert_eq!(r.code, 15, "{modele}");
        assert!(r
            .erreur
            .starts_with("PARAVENT_ERREUR code=MODELE_INVALIDE : "));
        assert_eq!(r.sortie, "");
    }
    let r = paravent(
        &d.0,
        &[
            "modele",
            "--declaration",
            "declaration.json",
            "--type",
            "élève",
        ],
        b"Bonjour",
    );
    assert_eq!(r.code, 15);
}

#[test]
fn le_binaire_ne_sait_pas_remplir() {
    let d = Dossier::equipe();
    for commande in ["remplir", "executer", "valider"] {
        assert_eq!(paravent(&d.0, &[commande], b"").code, 2);
        assert_eq!(paravent(&d.0, &["modele", commande], b"").code, 2);
    }
    assert_eq!(
        paravent(&d.0, &["modele", "--type", "contact"], b"").code,
        2
    );
}
