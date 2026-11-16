//! Le binaire `paravent`. Codes de sortie et messages : `docs/codes.md`.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use paravent::filtre::Filtre;
use paravent::{Cle, Declaration, Fabrique};

const AIDE: &str = "\
paravent — filtre entre un agent et des données personnelles

  paravent filtrer --declaration <fichier> [--cle <fichier>] [--json]
      Lit des objets JSON sur l'entrée, un par ligne, et écrit leur forme sûre.
      Clé : --cle, sinon $PARAVENT_CLE, sinon ~/.config/paravent/cle.

  paravent modele --declaration <fichier> --type <type>
      Contrôle un modèle à trous lu sur l'entrée et rend les champs qu'il utilise.
      Ne remplit rien : le remplissage se fait du côté de la source.

  paravent banc --plan <fichier>
      Joue les commandes d'un outil et cherche les valeurs pièges du plan partout où
      l'outil écrit. Sortie 10 à la première fuite.

  paravent --version
  paravent aide

Codes de sortie et messages : docs/codes.md
";

/// Une erreur du binaire : un code stable, un numéro de sortie, un message sans donnée.
struct Echec {
    code: &'static str,
    sortie: u8,
    ligne: Option<usize>,
    message: String,
}

impl Echec {
    fn new(code: &'static str, sortie: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            sortie,
            ligne: None,
            message: message.into(),
        }
    }

    fn usage(message: impl Into<String>) -> Self {
        Self::new("USAGE", 2, message)
    }

    fn entree_sortie() -> Self {
        Self::new("ENTREE_SORTIE", 9, "lecture ou écriture impossible")
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match lancer(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let ligne = e.ligne.map(|n| format!(" ligne={n}")).unwrap_or_default();
            eprintln!("PARAVENT_ERREUR code={}{ligne} : {}", e.code, e.message);
            ExitCode::from(e.sortie)
        }
    }
}

fn lancer(arguments: &[String]) -> Result<(), Echec> {
    match arguments.first().map(String::as_str) {
        Some("filtrer") => filtrer(&arguments[1..]),
        Some("banc") => banc(&arguments[1..]),
        Some("modele") => modele(&arguments[1..]),
        Some("--version") => {
            println!("paravent {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("aide" | "--help" | "-h") => {
            print!("{AIDE}");
            Ok(())
        }
        _ => Err(Echec::usage(
            "commande attendue : filtrer, modele, banc, aide",
        )),
    }
}

fn filtrer(arguments: &[String]) -> Result<(), Echec> {
    let mut declaration: Option<PathBuf> = None;
    let mut cle: Option<PathBuf> = None;
    let mut json = false;
    let mut reste = arguments.iter();
    while let Some(argument) = reste.next() {
        match argument.as_str() {
            "--declaration" => declaration = Some(valeur(&mut reste, "--declaration")?),
            "--cle" => cle = Some(valeur(&mut reste, "--cle")?),
            "--json" => json = true,
            _ => return Err(Echec::usage("option inconnue pour filtrer")),
        }
    }
    let declaration = declaration.ok_or_else(|| Echec::usage("--declaration est obligatoire"))?;
    let declaration = Declaration::lire_fichier(&declaration)
        .map_err(|e| Echec::new("DECLARATION_INVALIDE", 3, e.to_string()))?;
    let cle = Cle::lire_fichier(&chemin_de_cle(cle)?)
        .map_err(|e| Echec::new("CLE_REFUSEE", 4, e.to_string()))?;
    let mut filtre = Filtre::new(declaration, Fabrique::new(cle));

    let mut entree = io::stdin().lock();
    let mut sortie = io::stdout().lock();
    let mut tampon = Vec::new();
    let mut numero = 0usize;
    loop {
        tampon.clear();
        let lus = entree
            .read_until(b'\n', &mut tampon)
            .map_err(|_| Echec::entree_sortie())?;
        if lus == 0 {
            break;
        }
        numero += 1;
        let illisible = paravent::filtre::ErreurFiltre::LigneIllisible;
        let objet = std::str::from_utf8(&tampon)
            .map_err(|_| illisible)
            .and_then(|ligne| filtre.traiter_ligne(ligne))
            .map_err(|e| Echec {
                code: e.code(),
                sortie: sortie_du_filtre(e),
                ligne: Some(numero),
                message: e.to_string(),
            })?;
        if let Some(objet) = objet {
            let texte = if json {
                objet.en_json().to_string()
            } else {
                objet.en_texte()
            };
            writeln!(sortie, "{texte}").map_err(|_| Echec::entree_sortie())?;
        }
    }
    let comptes = if json {
        filtre.comptes().en_json().to_string()
    } else {
        filtre.comptes().en_texte()
    };
    writeln!(sortie, "{comptes}").map_err(|_| Echec::entree_sortie())?;
    sortie.flush().map_err(|_| Echec::entree_sortie())
}

fn modele(arguments: &[String]) -> Result<(), Echec> {
    let mut declaration: Option<PathBuf> = None;
    let mut type_objet: Option<&String> = None;
    let mut reste = arguments.iter();
    while let Some(argument) = reste.next() {
        match argument.as_str() {
            "--declaration" => declaration = Some(valeur(&mut reste, "--declaration")?),
            "--type" => {
                type_objet = Some(
                    reste
                        .next()
                        .ok_or_else(|| Echec::usage("--type attend un nom de type"))?,
                )
            }
            _ => return Err(Echec::usage("option inconnue pour modele")),
        }
    }
    let declaration = declaration.ok_or_else(|| Echec::usage("--declaration est obligatoire"))?;
    let type_objet = type_objet.ok_or_else(|| Echec::usage("--type est obligatoire"))?;
    let declaration = Declaration::lire_fichier(&declaration)
        .map_err(|e| Echec::new("DECLARATION_INVALIDE", 3, e.to_string()))?;
    let mut texte = String::new();
    io::Read::read_to_string(&mut io::stdin().lock(), &mut texte)
        .map_err(|_| Echec::entree_sortie())?;
    let invalide =
        |e: paravent::modele::ErreurModele| Echec::new("MODELE_INVALIDE", 15, e.to_string());
    let controle = paravent::modele::Modele::analyser(&texte)
        .map_err(invalide)?
        .controler(&declaration, type_objet)
        .map_err(invalide)?;
    println!(
        "MODELE_OK type={} champs={}",
        controle.type_objet(),
        controle.champs().join(",")
    );
    Ok(())
}

fn banc(arguments: &[String]) -> Result<(), Echec> {
    let plan = match arguments {
        [option, chemin] if option == "--plan" => PathBuf::from(chemin),
        _ => return Err(Echec::usage("usage : paravent banc --plan <fichier>")),
    };
    let echec = |e: paravent::banc::ErreurBanc| {
        use paravent::banc::ErreurBanc::*;
        let sortie = match e {
            CommandeInlancable { .. } => 12,
            CommandeTropLongue { .. } => 14,
            _ => 11,
        };
        Echec::new(e.code(), sortie, e.to_string())
    };
    let plan = paravent::banc::Plan::lire_fichier(&plan).map_err(echec)?;
    let rapport = paravent::banc::jouer(&plan).map_err(echec)?;
    for fuite in &rapport.fuites {
        println!("{fuite}");
    }
    let (commandes, pieges) = (rapport.commandes, rapport.pieges);
    if rapport.fuites.is_empty() {
        println!("BANC_OK commandes={commandes} pieges={pieges}");
        return Ok(());
    }
    let fuites = rapport.fuites.len();
    println!("BANC_ECHEC commandes={commandes} pieges={pieges} fuites={fuites}");
    Err(Echec::new(
        "FUITE",
        10,
        "au moins une valeur piège est sortie",
    ))
}

fn sortie_du_filtre(e: paravent::filtre::ErreurFiltre) -> u8 {
    use paravent::filtre::ErreurFiltre::*;
    match e {
        LigneIllisible => 5,
        TypeInconnu => 6,
        ChampSecret => 7,
        Collision => 8,
        ReferenceInvalide => 13,
    }
}

fn valeur<'a>(
    reste: &mut impl Iterator<Item = &'a String>,
    option: &str,
) -> Result<PathBuf, Echec> {
    reste
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| Echec::usage(format!("{option} attend un fichier")))
}

fn chemin_de_cle(donne: Option<PathBuf>) -> Result<PathBuf, Echec> {
    if let Some(chemin) = donne {
        return Ok(chemin);
    }
    if let Some(chemin) = std::env::var_os("PARAVENT_CLE").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(chemin));
    }
    std::env::var_os("HOME")
        .map(|maison| PathBuf::from(maison).join(".config/paravent/cle"))
        .ok_or_else(|| Echec::new("CLE_REFUSEE", 4, "aucun fichier de clé indiqué"))
}
