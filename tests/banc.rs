//! Le banc anti-fuite se prouve lui-même : un outil fuyant le fait échouer, le même outil
//! branché sur le filtre le fait passer.
#![cfg(unix)]

mod commun;

use commun::{paravent, Dossier, Resultat};

const PIEGES: [&str; 2] = ["PIEGE-prenom-7f3a", "piege.courriel@exemple.invalid"];

const DONNEES: &str = concat!(
    r#"{"type":"contact","id":"1042","champs":{"prénom":"PIEGE-prenom-7f3a","courriel":"piege.courriel@exemple.invalid","rôle":"enseignante","état":"actif"}}"#,
    "\n",
);

/// L'outil branché : il envoie ses données au filtre au lieu de les écrire.
const BRANCHE: &str =
    r#""$PARAVENT" filtrer --declaration declaration.json --cle cle < donnees.jsonl"#;

fn banc(commandes: &[&str], surveiller: &[&str]) -> Resultat {
    let d = Dossier::equipe();
    d.ecrire("donnees.jsonl", DONNEES);
    std::fs::create_dir(d.chemin("sorties")).unwrap();
    let commandes: Vec<Vec<&str>> = commandes.iter().map(|c| vec!["sh", "-c", c]).collect();
    let plan = serde_json::json!({
        "pieges": PIEGES,
        "commandes": commandes,
        "surveiller": surveiller,
    });
    d.ecrire("plan.json", &plan.to_string());
    let r = paravent(&d.0, &["banc", "--plan", "plan.json"], b"");
    for piege in PIEGES {
        assert!(!r.sortie.contains(piege), "le banc a réaffiché un piège");
        assert!(!r.erreur.contains(piege), "le banc a réaffiché un piège");
    }
    r
}

fn echoue_avec(r: &Resultat, fuite: &str) {
    assert_eq!(r.code, 10, "{}{}", r.sortie, r.erreur);
    assert!(
        r.sortie.contains(fuite),
        "attendu « {fuite} » dans :\n{}",
        r.sortie
    );
    assert!(r.sortie.contains("BANC_ECHEC"));
    assert!(r.erreur.starts_with("PARAVENT_ERREUR code=FUITE"));
}

#[test]
fn un_outil_branche_sur_le_filtre_passe() {
    let r = banc(&[BRANCHE, &format!("{BRANCHE} --json")], &["sorties"]);
    assert_eq!(r.code, 0, "{}{}", r.sortie, r.erreur);
    assert_eq!(r.sortie, "BANC_OK commandes=2 pieges=2\n");
}

#[test]
fn un_outil_qui_affiche_ses_donnees_echoue() {
    let r = banc(&["cat donnees.jsonl"], &[]);
    echoue_avec(
        &r,
        "FUITE commande=1 emplacement=stdout piege=1 variante=clair",
    );
    echoue_avec(
        &r,
        "FUITE commande=1 emplacement=stdout piege=2 variante=clair",
    );
    assert!(r
        .sortie
        .contains("BANC_ECHEC commandes=1 pieges=2 fuites=2"));
}

#[test]
fn une_seule_commande_fuyante_suffit_et_elle_est_nommee() {
    let r = banc(&[BRANCHE, "cat donnees.jsonl >&2", BRANCHE], &[]);
    echoue_avec(
        &r,
        "FUITE commande=2 emplacement=stderr piege=1 variante=clair",
    );
    assert!(!r.sortie.contains("commande=1 "));
    assert!(!r.sortie.contains("commande=3 "));
}

#[test]
fn une_fuite_dans_un_fichier_cree_ou_dans_le_journal_est_trouvee() {
    let r = banc(&["cp donnees.jsonl sorties/export.txt"], &["sorties"]);
    echoue_avec(
        &r,
        "FUITE commande=1 emplacement=fichier:sorties/export.txt piege=1 variante=clair",
    );

    let r = banc(
        &[
            "echo debut >> outil.log",
            "grep -o 'piege[^\"]*' donnees.jsonl >> outil.log",
        ],
        &["outil.log"],
    );
    echoue_avec(
        &r,
        "FUITE commande=2 emplacement=fichier:outil.log piege=2 variante=clair",
    );
    assert!(!r.sortie.contains("commande=1 "));
}

#[test]
fn un_fichier_deja_la_et_inchange_n_est_pas_une_fuite() {
    // Le jeu de données lui-même contient les pièges : il est surveillé, mais personne n'y écrit.
    let r = banc(&[BRANCHE], &["donnees.jsonl", "sorties"]);
    assert_eq!(r.code, 0, "{}{}", r.sortie, r.erreur);
}

#[test]
fn un_piege_dans_un_nom_de_fichier_est_une_fuite_et_le_nom_est_masque() {
    let r = banc(&["echo ok > sorties/PIEGE-prenom-7f3a.txt"], &["sorties"]);
    echoue_avec(
        &r,
        "FUITE commande=1 emplacement=nom-de-fichier piege=1 variante=clair",
    );
}

#[test]
fn les_variantes_simples_sont_trouvees() {
    let r = banc(&["tr a-z A-Z < donnees.jsonl"], &[]);
    echoue_avec(&r, "emplacement=stdout piege=1 variante=casse");

    let r = banc(&["base64 < donnees.jsonl"], &[]);
    echoue_avec(&r, "emplacement=stdout piege=1 variante=base64");
    echoue_avec(&r, "emplacement=stdout piege=2 variante=base64");

    let r = banc(&["echo 'a=piege.courriel%40exemple.invalid'"], &[]);
    echoue_avec(&r, "emplacement=stdout piege=2 variante=url");
}

#[test]
fn une_commande_qui_echoue_est_cherchee_comme_les_autres() {
    let r = banc(&["cat donnees.jsonl >&2; exit 3"], &[]);
    echoue_avec(&r, "FUITE commande=1 emplacement=stderr piege=1");
    let r = banc(&["exit 3"], &[]);
    assert_eq!(r.code, 0);
}

#[test]
fn un_plan_invalide_ou_une_commande_introuvable_ont_leur_code() {
    let d = Dossier::equipe();
    let r = paravent(&d.0, &["banc", "--plan", "absent.json"], b"");
    assert_eq!(r.code, 11);
    assert!(r.erreur.starts_with("PARAVENT_ERREUR code=PLAN_INVALIDE"));

    d.ecrire(
        "plan.json",
        r#"{"pieges": ["PIEGE-prenom-7f3a"], "commandes": [["commande-qui-n-existe-pas"]]}"#,
    );
    let r = paravent(&d.0, &["banc", "--plan", "plan.json"], b"");
    assert_eq!(r.code, 12);
    assert!(r
        .erreur
        .starts_with("PARAVENT_ERREUR code=COMMANDE_INLANCABLE"));

    d.ecrire(
        "plan.json",
        r#"{"pieges": ["PIEGE-prenom-7f3a"], "commandes": [["sleep", "30"]], "delai_secondes": 1}"#,
    );
    let r = paravent(&d.0, &["banc", "--plan", "plan.json"], b"");
    assert_eq!(r.code, 14);
    assert!(r
        .erreur
        .starts_with("PARAVENT_ERREUR code=COMMANDE_TROP_LONGUE"));

    assert_eq!(paravent(&d.0, &["banc"], b"").code, 2);
}
