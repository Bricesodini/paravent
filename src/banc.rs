//! Le banc anti-fuite : joue les commandes d'un outil et cherche des valeurs pièges partout où
//! l'outil écrit.
//!
//! Le plan (JSON) :
//!
//! ```json
//! {
//!   "pieges": ["PIEGE-prenom-7f3a", "piege.courriel@exemple.invalid"],
//!   "commandes": [["outil", "lister"], ["outil", "chercher", "PIEGE-prenom-7f3a"]],
//!   "surveiller": ["sorties", "outil.log"],
//!   "delai_secondes": 60
//! }
//! ```
//!
//! Les pièges sont des valeurs reconnaissables, semées dans le jeu de données de l'outil. Les
//! commandes sont lancées depuis le dossier du plan, sans interpréteur. `surveiller` liste les
//! fichiers et dossiers où l'outil écrit : résultats, journal.
//!
//! Aucune sortie du banc ne recopie un piège ni ce qu'une commande a écrit.

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Longueur minimale d'un piège, en octets : plus court, il se rencontrerait par hasard.
pub const LONGUEUR_MIN_PIEGE: usize = 8;

const DELAI_PAR_DEFAUT: u64 = 60;

/// Pourquoi le banc n'a pas pu se jouer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurBanc {
    /// Le plan ne se lit pas.
    PlanInaccessible,
    /// Le plan n'est pas au format attendu.
    PlanIllisible { ligne: usize, colonne: usize },
    /// Le plan ne liste aucun piège ou aucune commande, ou une commande est vide.
    PlanVide,
    /// Un piège fait moins de [`LONGUEUR_MIN_PIEGE`] octets.
    PiegeTropCourt { piege: usize },
    /// Une commande n'a pas pu être lancée.
    CommandeInlancable { commande: usize },
    /// Une commande a dépassé le délai.
    CommandeTropLongue { commande: usize },
}

impl ErreurBanc {
    pub fn code(&self) -> &'static str {
        match self {
            Self::PlanInaccessible
            | Self::PlanIllisible { .. }
            | Self::PlanVide
            | Self::PiegeTropCourt { .. } => "PLAN_INVALIDE",
            Self::CommandeInlancable { .. } => "COMMANDE_INLANCABLE",
            Self::CommandeTropLongue { .. } => "COMMANDE_TROP_LONGUE",
        }
    }
}

impl fmt::Display for ErreurBanc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanInaccessible => write!(f, "le plan ne se lit pas"),
            Self::PlanIllisible { ligne, colonne } => {
                write!(f, "plan illisible (ligne {ligne}, colonne {colonne})")
            }
            Self::PlanVide => write!(
                f,
                "le plan doit lister au moins un piège et une commande non vide"
            ),
            Self::PiegeTropCourt { piege } => write!(
                f,
                "le piège n° {piege} fait moins de {LONGUEUR_MIN_PIEGE} octets"
            ),
            Self::CommandeInlancable { commande } => {
                write!(f, "la commande n° {commande} n'a pas pu être lancée")
            }
            Self::CommandeTropLongue { commande } => {
                write!(f, "la commande n° {commande} a dépassé le délai")
            }
        }
    }
}

impl std::error::Error for ErreurBanc {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanBrut {
    pieges: Vec<String>,
    commandes: Vec<Vec<String>>,
    #[serde(default)]
    surveiller: Vec<String>,
    #[serde(default)]
    delai_secondes: Option<u64>,
}

/// Un plan lu et vérifié. Son `Debug` ne montre pas les pièges.
pub struct Plan {
    dossier: PathBuf,
    pieges: Vec<String>,
    commandes: Vec<Vec<String>>,
    surveiller: Vec<PathBuf>,
    delai: Duration,
}

impl fmt::Debug for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Plan(pièges : {}, commandes : {})",
            self.pieges.len(),
            self.commandes.len()
        )
    }
}

impl Plan {
    /// Lit un plan ; `dossier` est celui d'où partent les commandes et les chemins surveillés.
    pub fn lire(texte: &str, dossier: &Path) -> Result<Self, ErreurBanc> {
        let brut: PlanBrut =
            serde_json::from_str(texte).map_err(|e| ErreurBanc::PlanIllisible {
                ligne: e.line(),
                colonne: e.column(),
            })?;
        if brut.pieges.is_empty()
            || brut.commandes.is_empty()
            || brut.commandes.iter().any(|c| c.is_empty())
        {
            return Err(ErreurBanc::PlanVide);
        }
        if let Some(i) = brut
            .pieges
            .iter()
            .position(|p| p.len() < LONGUEUR_MIN_PIEGE)
        {
            return Err(ErreurBanc::PiegeTropCourt { piege: i + 1 });
        }
        Ok(Self {
            dossier: dossier.to_path_buf(),
            pieges: brut.pieges,
            commandes: brut.commandes,
            surveiller: brut.surveiller.iter().map(|s| dossier.join(s)).collect(),
            delai: Duration::from_secs(brut.delai_secondes.unwrap_or(DELAI_PAR_DEFAUT)),
        })
    }

    /// Lit un plan depuis un fichier ; les commandes partent du dossier de ce fichier.
    pub fn lire_fichier(chemin: &Path) -> Result<Self, ErreurBanc> {
        let texte = std::fs::read_to_string(chemin).map_err(|_| ErreurBanc::PlanInaccessible)?;
        let dossier = match chemin.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        Self::lire(&texte, dossier)
    }
}

/// Où un piège a été trouvé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Emplacement {
    SortieStandard,
    SortieErreur,
    /// Le contenu d'un fichier créé ou modifié par la commande.
    Fichier(String),
    /// Le nom même d'un fichier créé ou modifié par la commande.
    NomDeFichier,
}

impl fmt::Display for Emplacement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SortieStandard => f.write_str("stdout"),
            Self::SortieErreur => f.write_str("stderr"),
            Self::Fichier(chemin) => write!(f, "fichier:{chemin}"),
            Self::NomDeFichier => f.write_str("nom-de-fichier"),
        }
    }
}

/// Une fuite : quelle commande, où, quel piège (par son rang dans le plan), sous quelle forme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fuite {
    pub commande: usize,
    pub emplacement: Emplacement,
    pub piege: usize,
    pub variante: &'static str,
}

impl fmt::Display for Fuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FUITE commande={} emplacement={} piege={} variante={}",
            self.commande, self.emplacement, self.piege, self.variante
        )
    }
}

/// Le résultat d'un banc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rapport {
    pub commandes: usize,
    pub pieges: usize,
    pub fuites: Vec<Fuite>,
}

/// Joue le plan : chaque commande, puis la recherche de chaque piège là où elle a écrit.
pub fn jouer(plan: &Plan) -> Result<Rapport, ErreurBanc> {
    let chercheur = Chercheur::new(&plan.pieges);
    let mut fuites = Vec::new();
    for (i, commande) in plan.commandes.iter().enumerate() {
        let numero = i + 1;
        let avant = empreintes(&plan.surveiller);
        let (sortie, erreur) = lancer(commande, &plan.dossier, plan.delai, numero)?;
        let mut noter = |emplacement: Emplacement, octets: &[u8]| {
            for (piege, variante) in chercheur.chercher(octets) {
                fuites.push(Fuite {
                    commande: numero,
                    emplacement: emplacement.clone(),
                    piege,
                    variante,
                });
            }
        };
        noter(Emplacement::SortieStandard, &sortie);
        noter(Emplacement::SortieErreur, &erreur);
        for (chemin, empreinte) in empreintes(&plan.surveiller) {
            if avant.get(&chemin) == Some(&empreinte) {
                continue;
            }
            let relatif = chemin
                .strip_prefix(&plan.dossier)
                .unwrap_or(&chemin)
                .to_string_lossy()
                .into_owned();
            // Un nom de fichier qui porte un piège est une fuite, et ne s'affiche pas.
            let nom_piege = !chercheur.chercher(relatif.as_bytes()).is_empty();
            noter(Emplacement::NomDeFichier, relatif.as_bytes());
            let affiche = if nom_piege {
                "(nom masqué)".to_string()
            } else {
                relatif
            };
            if let Ok(contenu) = std::fs::read(&chemin) {
                noter(Emplacement::Fichier(affiche), &contenu);
            }
        }
    }
    Ok(Rapport {
        commandes: plan.commandes.len(),
        pieges: plan.pieges.len(),
        fuites,
    })
}

/// Lance une commande sans interpréteur et rend ce qu'elle a écrit sur ses deux sorties.
fn lancer(
    commande: &[String],
    dossier: &Path,
    delai: Duration,
    numero: usize,
) -> Result<(Vec<u8>, Vec<u8>), ErreurBanc> {
    let inlancable = || ErreurBanc::CommandeInlancable { commande: numero };
    let mut enfant = Command::new(&commande[0])
        .args(&commande[1..])
        .current_dir(dossier)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| inlancable())?;
    // Les deux sorties se lisent pendant que la commande tourne : sinon un tube plein la bloque.
    let lire = |flux: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut octets = Vec::new();
            if let Some(mut flux) = flux {
                let _ = flux.read_to_end(&mut octets);
            }
            octets
        })
    };
    let sortie = lire(enfant.stdout.take().map(|f| Box::new(f) as _));
    let erreur = lire(enfant.stderr.take().map(|f| Box::new(f) as _));
    let debut = Instant::now();
    loop {
        match enfant.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if debut.elapsed() > delai => {
                let _ = enfant.kill();
                let _ = enfant.wait();
                return Err(ErreurBanc::CommandeTropLongue { commande: numero });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return Err(inlancable()),
        }
    }
    Ok((
        sortie.join().unwrap_or_default(),
        erreur.join().unwrap_or_default(),
    ))
}

/// L'empreinte de chaque fichier sous les chemins surveillés. Les liens ne sont pas suivis.
fn empreintes(racines: &[PathBuf]) -> BTreeMap<PathBuf, [u8; 32]> {
    fn visiter(chemin: &Path, vus: &mut BTreeMap<PathBuf, [u8; 32]>) {
        let Ok(meta) = std::fs::symlink_metadata(chemin) else {
            return;
        };
        if meta.is_dir() {
            if let Ok(entrees) = std::fs::read_dir(chemin) {
                for entree in entrees.flatten() {
                    visiter(&entree.path(), vus);
                }
            }
        } else if meta.is_file() {
            if let Ok(contenu) = std::fs::read(chemin) {
                vus.insert(chemin.to_path_buf(), Sha256::digest(&contenu).into());
            }
        }
    }
    let mut vus = BTreeMap::new();
    for racine in racines {
        visiter(racine, &mut vus);
    }
    vus
}

/// Une forme sous laquelle un piège peut sortir.
struct Aiguille {
    piege: usize,
    variante: &'static str,
    octets: Vec<u8>,
    /// Vrai pour le base64 : sensible à la casse, cherché sans les retours à la ligne.
    base64: bool,
}

/// Cherche tous les pièges, sous toutes leurs variantes simples.
struct Chercheur {
    aiguilles: Vec<Aiguille>,
    /// Les pièges tels quels, pour dire si une occurrence est exacte ou dans une autre casse.
    exacts: Vec<Vec<u8>>,
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

impl Chercheur {
    fn new(pieges: &[String]) -> Self {
        let mut aiguilles = Vec::new();
        for (i, piege) in pieges.iter().enumerate() {
            let mut ajouter = |variante, octets: Vec<u8>, base64| {
                aiguilles.push(Aiguille {
                    piege: i + 1,
                    variante,
                    octets,
                    base64,
                });
            };
            // La casse : tout se compare en minuscules ASCII, et dans les deux casses Unicode.
            let casses = [piege.clone(), piege.to_lowercase(), piege.to_uppercase()];
            for (rang, forme) in casses.iter().enumerate() {
                let variante = if rang == 0 { "clair" } else { "casse" };
                ajouter(variante, forme.as_bytes().to_ascii_lowercase(), false);
                ajouter("url", encoder_url(forme, false).to_ascii_lowercase(), false);
                ajouter("url", encoder_url(forme, true).to_ascii_lowercase(), false);
                ajouter("json", echapper_json(forme).to_ascii_lowercase(), false);
            }
            for alphabet in [BASE64, BASE64_URL] {
                for decalage in 0..3 {
                    ajouter(
                        "base64",
                        fragment_base64(piege.as_bytes(), decalage, alphabet),
                        true,
                    );
                }
            }
        }
        Self {
            aiguilles,
            exacts: pieges.iter().map(|p| p.as_bytes().to_vec()).collect(),
        }
    }

    /// Les pièges présents dans `octets` : rang du piège et première variante trouvée.
    fn chercher(&self, octets: &[u8]) -> Vec<(usize, &'static str)> {
        let minuscules = octets.to_ascii_lowercase();
        let compact: Vec<u8> = octets
            .iter()
            .copied()
            .filter(|o| !matches!(o, b'\r' | b'\n'))
            .collect();
        let mut trouves: Vec<(usize, &'static str)> = Vec::new();
        for aiguille in &self.aiguilles {
            if trouves.iter().any(|(piege, _)| *piege == aiguille.piege) {
                continue;
            }
            let botte = if aiguille.base64 {
                &compact
            } else {
                &minuscules
            };
            if contient(botte, &aiguille.octets) {
                // La comparaison ignore la casse : « clair » se réserve à l'occurrence exacte.
                let exact = contient(octets, &self.exacts[aiguille.piege - 1]);
                let variante = match aiguille.variante {
                    "clair" | "casse" if exact => "clair",
                    "clair" => "casse",
                    autre => autre,
                };
                trouves.push((aiguille.piege, variante));
            }
        }
        trouves
    }
}

fn contient(botte: &[u8], aiguille: &[u8]) -> bool {
    !aiguille.is_empty() && botte.windows(aiguille.len()).any(|f| f == aiguille)
}

/// Encodage URL : tout octet hors lettres, chiffres et `-._~` devient `%XX`.
fn encoder_url(texte: &str, espace_en_plus: bool) -> Vec<u8> {
    let mut sortie = Vec::new();
    for octet in texte.bytes() {
        match octet {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                sortie.push(octet)
            }
            b' ' if espace_en_plus => sortie.push(b'+'),
            _ => sortie.extend(format!("%{octet:02X}").bytes()),
        }
    }
    sortie
}

/// Échappement JSON des caractères non ASCII (`\uXXXX`), comme l'écrivent bien des outils.
fn echapper_json(texte: &str) -> Vec<u8> {
    let mut sortie = Vec::new();
    let mut tampon = [0u16; 2];
    for c in texte.chars() {
        if c.is_ascii() {
            sortie.push(c as u8);
        } else {
            for unite in c.encode_utf16(&mut tampon) {
                sortie.extend(format!("\\u{unite:04x}").bytes());
            }
        }
    }
    sortie
}

/// Base64 sans remplissage.
fn base64(octets: &[u8], alphabet: &[u8; 64]) -> Vec<u8> {
    let mut sortie = Vec::new();
    for bloc in octets.chunks(3) {
        let n = bloc
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, o)| acc | (u32::from(*o) << (16 - 8 * i)));
        for i in 0..=bloc.len() {
            sortie.push(alphabet[((n >> (18 - 6 * i)) & 63) as usize]);
        }
    }
    sortie
}

/// La partie du base64 d'un piège qui ne dépend pas de ce qui l'entoure, quand il commence
/// `decalage` octets après une frontière de trois octets.
fn fragment_base64(piege: &[u8], decalage: usize, alphabet: &[u8; 64]) -> Vec<u8> {
    let mut octets = vec![0u8; decalage];
    octets.extend_from_slice(piege);
    let encode = base64(&octets, alphabet);
    let debut = (decalage * 8).div_ceil(6);
    let fin = octets.len() * 8 / 6;
    encode[debut..fin].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIEGE: &str = "PIEGE-prénom Élodie";

    fn variante(texte: &[u8]) -> Option<&'static str> {
        Chercheur::new(&[PIEGE.to_string()])
            .chercher(texte)
            .first()
            .map(|(_, v)| *v)
    }

    #[test]
    fn base64_suit_les_vecteurs_connus() {
        assert_eq!(base64(b"Man", BASE64), b"TWFu");
        assert_eq!(base64(b"Ma", BASE64), b"TWE");
        assert_eq!(base64(b"M", BASE64), b"TQ");
        assert_eq!(base64(&[0xfb, 0xff], BASE64_URL), b"-_8");
    }

    #[test]
    fn trouve_le_piege_en_clair_et_dans_une_autre_casse() {
        assert_eq!(
            variante(format!("avant {PIEGE} après").as_bytes()),
            Some("clair")
        );
        assert_eq!(
            variante("x PIEGE-PRÉNOM ÉLODIE x".as_bytes()),
            Some("casse")
        );
        assert_eq!(
            variante("x piege-prénom élodie x".as_bytes()),
            Some("casse")
        );
        assert_eq!(
            variante("x Piege-Prénom Élodie x".as_bytes()),
            Some("casse")
        );
    }

    #[test]
    fn trouve_le_piege_encode_pour_une_url() {
        assert_eq!(
            variante(b"q=PIEGE-pr%C3%A9nom%20%C3%89lodie&x=1"),
            Some("url")
        );
        assert_eq!(variante(b"q=piege-pr%c3%a9nom+%c3%89lodie"), Some("url"));
    }

    #[test]
    fn trouve_le_piege_echappe_en_json() {
        assert_eq!(
            variante(br#"{"p": "PIEGE-pr\u00e9nom \u00C9lodie"}"#),
            Some("json")
        );
    }

    #[test]
    fn trouve_le_piege_en_base64_quel_que_soit_son_alignement() {
        for avant in ["", "a", "ab", "abc", "abcd"] {
            let texte = format!("{avant}{PIEGE} et la suite");
            for alphabet in [BASE64, BASE64_URL] {
                let mut encode = base64(texte.as_bytes(), alphabet);
                assert_eq!(variante(&encode), Some("base64"), "préfixe « {avant} »");
                // Coupé en lignes, comme le font les outils usuels.
                encode.insert(10, b'\n');
                encode.insert(20, b'\r');
                assert_eq!(variante(&encode), Some("base64"));
            }
        }
    }

    #[test]
    fn ne_trouve_rien_dans_un_texte_sain() {
        assert_eq!(
            variante("contact-f8z00rja\trôle=\"enseignante\"".as_bytes()),
            None
        );
        assert_eq!(variante(b""), None);
    }

    #[test]
    fn refuse_un_plan_incomplet_sans_citer_ses_pieges() {
        let d = Path::new(".");
        let e = Plan::lire(r#"{"pieges": ["VALEUR-TEMOIN"], "commandes": "x"}"#, d).unwrap_err();
        assert!(matches!(e, ErreurBanc::PlanIllisible { .. }));
        assert!(!e.to_string().contains("VALEUR-TEMOIN"));
        for texte in [
            r#"{"pieges": [], "commandes": [["x"]]}"#,
            r#"{"pieges": ["VALEUR-TEMOIN"], "commandes": []}"#,
            r#"{"pieges": ["VALEUR-TEMOIN"], "commandes": [[]]}"#,
        ] {
            assert_eq!(Plan::lire(texte, d).unwrap_err(), ErreurBanc::PlanVide);
        }
        assert_eq!(
            Plan::lire(
                r#"{"pieges": ["VALEUR-TEMOIN", "court"], "commandes": [["x"]]}"#,
                d
            )
            .unwrap_err(),
            ErreurBanc::PiegeTropCourt { piege: 2 }
        );
        let plan = Plan::lire(r#"{"pieges": ["VALEUR-TEMOIN"], "commandes": [["x"]]}"#, d).unwrap();
        assert!(!format!("{plan:?}").contains("TEMOIN"));
    }
}
