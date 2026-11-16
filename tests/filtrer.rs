//! Le binaire `paravent filtrer`, de bout en bout.
#![cfg(unix)]

mod commun;

use commun::{paravent, Dossier};

const FILTRER: [&str; 5] = [
    "filtrer",
    "--declaration",
    "declaration.json",
    "--cle",
    "cle",
];

const DEUX_CONTACTS: &str = concat!(
    r#"{"type":"contact","id":"1042","champs":{"prénom":"Camille","courriel":"camille@exemple.invalid","rôle":"enseignante","état":"actif"}}"#,
    "\n\n",
    r#"{"type":"contact","id":"1043","champs":{"prénom":"Sacha","courriel":"","téléphone":"06 00 00 00 00","état":"actif"}}"#,
    "\n",
);

const VALEURS_PERSONNELLES: [&str; 6] = [
    "Camille",
    "camille@exemple.invalid",
    "Sacha",
    "06 00",
    "1042",
    "1043",
];

#[test]
fn ecrit_la_sortie_sure_et_les_comptes() {
    let d = Dossier::equipe();
    let r = paravent(&d.0, &FILTRER, DEUX_CONTACTS.as_bytes());
    assert_eq!(r.code, 0, "{}", r.erreur);
    assert_eq!(r.erreur, "");
    let lignes: Vec<&str> = r.sortie.lines().collect();
    assert_eq!(lignes.len(), 5, "{}", r.sortie);
    assert!(lignes[0].starts_with("contact-"));
    assert!(lignes[0].contains("rôle=\"enseignante\""));
    assert!(lignes[0].contains("courriel:renseigné"));
    assert!(lignes[1].contains("courriel:vide"));
    assert!(lignes[1].contains("téléphone:renseigné"));
    assert_eq!(
        lignes[2..],
        ["# comptes", "contact\t2", "contact.état=actif\t2"]
    );
    for valeur in VALEURS_PERSONNELLES {
        assert!(!r.sortie.contains(valeur), "« {valeur} » est sorti");
    }
}

#[test]
fn ecrit_du_json_sur_demande() {
    let d = Dossier::equipe();
    let mut arguments = FILTRER.to_vec();
    arguments.push("--json");
    let r = paravent(&d.0, &arguments, DEUX_CONTACTS.as_bytes());
    assert_eq!(r.code, 0, "{}", r.erreur);
    let lignes: Vec<serde_json::Value> = r
        .sortie
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lignes.len(), 3);
    assert_eq!(lignes[0]["type"], "contact");
    assert_eq!(lignes[0]["publics"]["état"], "actif");
    assert_eq!(lignes[0]["personnels"]["prénom"], "renseigné");
    assert_eq!(lignes[2]["comptes"]["contact"]["total"], 2);
    for valeur in VALEURS_PERSONNELLES {
        assert!(!r.sortie.contains(valeur), "« {valeur} » est sorti");
    }
}

#[test]
fn les_references_sont_stables_d_une_execution_a_l_autre() {
    let d = Dossier::equipe();
    let a = paravent(&d.0, &FILTRER, DEUX_CONTACTS.as_bytes());
    let b = paravent(&d.0, &FILTRER, DEUX_CONTACTS.as_bytes());
    assert_eq!(a.sortie, b.sortie);
}

/// Chaque refus : son code de sortie, son code stable, et rien de la ligne fautive.
#[test]
fn s_arrete_sur_un_refus_sans_recopier_la_ligne() {
    let cas = [
        (
            r#"{"type":"contact","id":"1","champs":{"prénom":"VALEUR-TEMOIN"}"#,
            5,
            "LIGNE_ILLISIBLE",
        ),
        (
            r#"{"type":"élève","id":"1","champs":{"nom":"VALEUR-TEMOIN"}}"#,
            6,
            "TYPE_INCONNU",
        ),
        (
            r#"{"type":"contact","id":"1","champs":{"jeton":"VALEUR-TEMOIN"}}"#,
            7,
            "CHAMP_SECRET",
        ),
        (
            r#"{"type":"contact","ref":"VALEUR-TEMOIN"}"#,
            13,
            "REFERENCE_INVALIDE",
        ),
    ];
    for (ligne, code, nom) in cas {
        let d = Dossier::equipe();
        let entree = format!(
            "{}{ligne}\n",
            r#"{"type":"contact","id":"9"}"#.to_owned() + "\n"
        );
        let r = paravent(&d.0, &FILTRER, entree.as_bytes());
        assert_eq!(r.code, code, "{nom} : {}", r.erreur);
        assert!(
            r.erreur
                .starts_with(&format!("PARAVENT_ERREUR code={nom} ligne=2 : ")),
            "{}",
            r.erreur
        );
        assert!(!r.erreur.contains("VALEUR-TEMOIN"), "{}", r.erreur);
        assert!(!r.sortie.contains("VALEUR-TEMOIN"), "{}", r.sortie);
        assert!(
            !r.sortie.contains("# comptes"),
            "pas de comptes après un refus"
        );
    }
}

#[test]
fn refuse_une_ligne_qui_n_est_pas_du_texte() {
    let d = Dossier::equipe();
    let r = paravent(&d.0, &FILTRER, &[0xff, 0xfe, b'\n']);
    assert_eq!(r.code, 5);
    assert!(r.erreur.contains("code=LIGNE_ILLISIBLE ligne=1"));
}

#[test]
fn refuse_une_declaration_invalide_ou_absente() {
    let d = Dossier::equipe();
    d.ecrire(
        "declaration.json",
        r#"{"version": 1, "types": "VALEUR-TEMOIN"}"#,
    );
    let r = paravent(&d.0, &FILTRER, b"");
    assert_eq!(r.code, 3);
    assert!(r
        .erreur
        .starts_with("PARAVENT_ERREUR code=DECLARATION_INVALIDE"));
    assert!(!r.erreur.contains("VALEUR-TEMOIN"));

    let r = paravent(
        &d.0,
        &["filtrer", "--declaration", "absente.json", "--cle", "cle"],
        b"",
    );
    assert_eq!(r.code, 3);
}

#[test]
fn refuse_une_cle_lisible_par_d_autres() {
    let d = Dossier::equipe();
    d.ecrire_cle("cle", 0o644);
    let r = paravent(&d.0, &FILTRER, DEUX_CONTACTS.as_bytes());
    assert_eq!(r.code, 4);
    assert!(r.erreur.starts_with("PARAVENT_ERREUR code=CLE_REFUSEE"));
    assert_eq!(r.sortie, "");
}

#[test]
fn un_usage_incorrect_a_son_code() {
    let d = Dossier::equipe();
    for arguments in [
        &[][..],
        &["inconnue"],
        &["filtrer"],
        &["filtrer", "--declaration"],
        &["filtrer", "--declaration", "declaration.json", "--bavard"],
    ] {
        let r = paravent(&d.0, arguments, b"");
        assert_eq!(r.code, 2, "{arguments:?}");
        assert!(r.erreur.starts_with("PARAVENT_ERREUR code=USAGE"));
    }
    assert_eq!(paravent(&d.0, &["--version"], b"").code, 0);
    assert_eq!(paravent(&d.0, &["aide"], b"").code, 0);
}
