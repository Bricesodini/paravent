//! Références opaques et stables (garantie 3).
//!
//! Une référence s'écrit `<type>-<suffixe>`. Le suffixe calculé tient en 8 caractères tirés d'un
//! HMAC-SHA-256 de `type` et `id` sous une clé propre à chaque installation. Format et choix :
//! `docs/adr/0001-format-des-references.md`.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::declaration::nom_de_type_valide;

/// Longueur minimale d'une clé, en octets.
pub const LONGUEUR_MIN_CLE: usize = 32;

/// Longueur du suffixe calculé, en caractères (40 bits).
pub const LONGUEUR_SUFFIXE: usize = 8;

/// Base 32 sans `i`, `l`, `o`, `u` : pas de confusion à la lecture ni à la dictée.
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// Pourquoi une clé est refusée. Aucun message ne contient la clé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurCle {
    /// Le fichier n'existe pas ou ne se lit pas.
    Inaccessible,
    /// Le chemin ne désigne pas un fichier ordinaire.
    PasUnFichier,
    /// Le fichier est lisible ou modifiable par d'autres que son propriétaire.
    TropOuverte,
    /// La clé fait moins de [`LONGUEUR_MIN_CLE`] octets.
    TropCourte,
    /// Ce système ne permet pas de vérifier les droits du fichier.
    DroitsInverifiables,
}

impl fmt::Display for ErreurCle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inaccessible => write!(f, "le fichier de clé ne se lit pas"),
            Self::PasUnFichier => write!(f, "la clé doit être un fichier ordinaire"),
            Self::TropOuverte => write!(
                f,
                "le fichier de clé doit n'être accessible qu'à son propriétaire (chmod 600)"
            ),
            Self::TropCourte => write!(f, "la clé doit faire au moins {LONGUEUR_MIN_CLE} octets"),
            Self::DroitsInverifiables => {
                write!(
                    f,
                    "les droits du fichier de clé ne sont pas vérifiables ici"
                )
            }
        }
    }
}

impl std::error::Error for ErreurCle {}

/// La clé d'une installation. Elle ne s'affiche pas et ne quitte pas ce module.
#[derive(Clone)]
pub struct Cle(Vec<u8>);

impl Cle {
    /// Construit une clé depuis ses octets.
    pub fn depuis_octets(octets: Vec<u8>) -> Result<Self, ErreurCle> {
        if octets.len() < LONGUEUR_MIN_CLE {
            return Err(ErreurCle::TropCourte);
        }
        Ok(Self(octets))
    }

    /// Lit la clé dans un fichier, refusé s'il est accessible à d'autres que son propriétaire.
    pub fn lire_fichier(chemin: &Path) -> Result<Self, ErreurCle> {
        let meta = std::fs::metadata(chemin).map_err(|_| ErreurCle::Inaccessible)?;
        if !meta.is_file() {
            return Err(ErreurCle::PasUnFichier);
        }
        verifier_droits(&meta)?;
        let octets = std::fs::read(chemin).map_err(|_| ErreurCle::Inaccessible)?;
        Self::depuis_octets(octets)
    }
}

#[cfg(unix)]
fn verifier_droits(meta: &std::fs::Metadata) -> Result<(), ErreurCle> {
    use std::os::unix::fs::PermissionsExt;
    if meta.permissions().mode() & 0o077 != 0 {
        return Err(ErreurCle::TropOuverte);
    }
    Ok(())
}

#[cfg(not(unix))]
fn verifier_droits(_meta: &std::fs::Metadata) -> Result<(), ErreurCle> {
    Err(ErreurCle::DroitsInverifiables)
}

impl fmt::Debug for Cle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cle(masquée)")
    }
}

/// Pourquoi une référence n'a pas pu être produite. Aucun message ne contient l'identifiant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurReference {
    /// Le type ne peut pas préfixer une référence.
    TypeInvalide,
    /// La référence fournie par la source n'a pas la forme `<type>-<suffixe>`.
    Malformee,
    /// Deux objets différents du même type donnent le même suffixe.
    Collision,
}

impl fmt::Display for ErreurReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeInvalide => write!(f, "type invalide pour une référence"),
            Self::Malformee => write!(
                f,
                "référence fournie mal formée : attendu <type>-<4 à 64 lettres minuscules ou chiffres>"
            ),
            Self::Collision => write!(
                f,
                "collision : deux objets du même type donnent la même référence"
            ),
        }
    }
}

impl std::error::Error for ErreurReference {}

/// Une référence : ce que l'agent voit à la place d'un identifiant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference {
    type_objet: String,
    suffixe: String,
}

impl Reference {
    /// Accepte une référence attribuée par la source elle-même (niveau 2).
    ///
    /// La forme est vérifiée ; l'opacité reste à la charge de la source, qui ne doit pas y
    /// recopier un identifiant d'origine.
    pub fn fournie(type_objet: &str, texte: &str) -> Result<Self, ErreurReference> {
        if !nom_de_type_valide(type_objet) {
            return Err(ErreurReference::TypeInvalide);
        }
        let suffixe = texte
            .strip_prefix(type_objet)
            .and_then(|reste| reste.strip_prefix('-'))
            .ok_or(ErreurReference::Malformee)?;
        let forme = (4..=64).contains(&suffixe.len())
            && suffixe
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        if !forme {
            return Err(ErreurReference::Malformee);
        }
        Ok(Self {
            type_objet: type_objet.to_string(),
            suffixe: suffixe.to_string(),
        })
    }

    pub fn type_objet(&self) -> &str {
        &self.type_objet
    }

    pub fn suffixe(&self) -> &str {
        &self.suffixe
    }
}

impl std::str::FromStr for Reference {
    type Err = ErreurReference;

    /// Relit une référence écrite `<type>-<suffixe>`. Le type ne contient pas de tiret.
    fn from_str(texte: &str) -> Result<Self, ErreurReference> {
        let (type_objet, _) = texte.split_once('-').ok_or(ErreurReference::Malformee)?;
        Self::fournie(type_objet, texte).map_err(|_| ErreurReference::Malformee)
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.type_objet, self.suffixe)
    }
}

type Empreinte = [u8; 32];

/// Fabrique les références d'une installation et détecte les collisions qu'elle voit passer.
///
/// La détection porte sur les objets vus par cette fabrique. Elle ne garde que des empreintes,
/// jamais d'identifiant.
#[derive(Debug)]
pub struct Fabrique {
    cle: Cle,
    vues: HashMap<(String, String), Empreinte>,
}

impl Fabrique {
    pub fn new(cle: Cle) -> Self {
        Self {
            cle,
            vues: HashMap::new(),
        }
    }

    /// La référence de l'objet `id` du type `type_objet`.
    pub fn reference(&mut self, type_objet: &str, id: &str) -> Result<Reference, ErreurReference> {
        if !nom_de_type_valide(type_objet) {
            return Err(ErreurReference::TypeInvalide);
        }
        let empreinte = self.empreinte(type_objet, id);
        let suffixe = suffixe(&empreinte);
        self.enregistrer(type_objet, suffixe, empreinte)
    }

    fn empreinte(&self, type_objet: &str, id: &str) -> Empreinte {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.cle.0).expect("HMAC accepte toute longueur");
        // La longueur du type sépare les deux parties : ("ab", "c") ne vaut pas ("a", "bc").
        mac.update(&(type_objet.len() as u64).to_be_bytes());
        mac.update(type_objet.as_bytes());
        mac.update(id.as_bytes());
        mac.finalize().into_bytes().into()
    }

    fn enregistrer(
        &mut self,
        type_objet: &str,
        suffixe: String,
        empreinte: Empreinte,
    ) -> Result<Reference, ErreurReference> {
        let cle = (type_objet.to_string(), suffixe);
        match self.vues.get(&cle) {
            Some(connue) if *connue != empreinte => return Err(ErreurReference::Collision),
            Some(_) => {}
            None => {
                self.vues.insert(cle.clone(), empreinte);
            }
        }
        Ok(Reference {
            type_objet: cle.0,
            suffixe: cle.1,
        })
    }
}

/// Les 40 premiers bits de l'empreinte, en base 32.
fn suffixe(empreinte: &Empreinte) -> String {
    let n = empreinte[..5]
        .iter()
        .fold(0u64, |acc, octet| (acc << 8) | u64::from(*octet));
    (0..LONGUEUR_SUFFIXE)
        .map(|i| ALPHABET[((n >> (35 - 5 * i)) & 31) as usize] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fabrique(graine: u8) -> Fabrique {
        Fabrique::new(Cle::depuis_octets(vec![graine; 32]).unwrap())
    }

    #[test]
    fn forme_type_tiret_huit_caracteres() {
        let r = fabrique(1).reference("contact", "1042").unwrap();
        let texte = r.to_string();
        let suffixe = texte.strip_prefix("contact-").unwrap();
        assert_eq!(suffixe.len(), 8);
        assert!(suffixe.bytes().all(|c| ALPHABET.contains(&c)));
        assert_eq!(r.type_objet(), "contact");
    }

    #[test]
    fn meme_objet_meme_reference() {
        let a = fabrique(1).reference("contact", "1042").unwrap();
        let b = fabrique(1).reference("contact", "1042").unwrap();
        assert_eq!(a, b);
        let mut f = fabrique(1);
        assert_eq!(f.reference("contact", "1042").unwrap(), a);
        assert_eq!(f.reference("contact", "1042").unwrap(), a);
    }

    #[test]
    fn deux_cles_deux_references() {
        let a = fabrique(1).reference("contact", "1042").unwrap();
        let b = fabrique(2).reference("contact", "1042").unwrap();
        assert_ne!(a.suffixe(), b.suffixe());
    }

    #[test]
    fn le_type_et_l_identifiant_ne_se_confondent_pas() {
        let mut f = fabrique(1);
        let a = f.reference("ab", "c").unwrap();
        let b = f.reference("a", "bc").unwrap();
        assert_ne!(a.suffixe(), b.suffixe());
        assert_ne!(
            f.reference("contact", "7").unwrap().suffixe(),
            f.reference("classe", "7").unwrap().suffixe()
        );
    }

    #[test]
    fn la_reference_ne_contient_pas_l_identifiant() {
        let mut f = fabrique(3);
        for id in ["104217", "camille.roche", "a1b2c3d4"] {
            let r = f.reference("contact", id).unwrap().to_string();
            assert!(!r.contains(id), "identifiant visible dans {r}");
        }
    }

    #[test]
    fn une_collision_est_signalee() {
        let mut f = fabrique(1);
        let r = f
            .enregistrer("contact", "aaaaaaaa".into(), [1; 32])
            .unwrap();
        assert_eq!(r.to_string(), "contact-aaaaaaaa");
        // Le même objet revient : pas de collision.
        assert!(f.enregistrer("contact", "aaaaaaaa".into(), [1; 32]).is_ok());
        // Un autre objet tombe sur le même suffixe.
        assert_eq!(
            f.enregistrer("contact", "aaaaaaaa".into(), [2; 32]),
            Err(ErreurReference::Collision)
        );
        // Le même suffixe dans un autre type ne gêne pas.
        assert!(f.enregistrer("classe", "aaaaaaaa".into(), [2; 32]).is_ok());
    }

    #[test]
    fn un_type_invalide_est_refuse() {
        assert_eq!(
            fabrique(1).reference("Mon-Type", "1"),
            Err(ErreurReference::TypeInvalide)
        );
    }

    #[test]
    fn une_source_peut_fournir_ses_references() {
        let r = Reference::fournie("contact", "contact-k3v9x2").unwrap();
        assert_eq!(r.to_string(), "contact-k3v9x2");
        for texte in [
            "contact-",
            "contact-abc",
            "classe-k3v9x2",
            "contact-K3V9X2",
            "contact-a b c d",
            "contactk3v9x2",
        ] {
            assert_eq!(
                Reference::fournie("contact", texte),
                Err(ErreurReference::Malformee),
                "« {texte} »"
            );
        }
    }

    #[test]
    fn une_cle_courte_est_refusee() {
        assert_eq!(
            Cle::depuis_octets(vec![7; 31]).unwrap_err(),
            ErreurCle::TropCourte
        );
    }

    #[test]
    fn la_cle_ne_s_affiche_pas() {
        let cle = Cle::depuis_octets(b"CLE-TEMOIN-CLE-TEMOIN-CLE-TEMOIN-".to_vec()).unwrap();
        assert_eq!(format!("{cle:?}"), "Cle(masquée)");
        assert!(!format!("{:?}", Fabrique::new(cle)).contains("TEMOIN"));
    }

    #[cfg(unix)]
    mod fichier {
        use super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        fn fichier_de_cle(nom: &str, contenu: &[u8], mode: u32) -> PathBuf {
            let chemin =
                std::env::temp_dir().join(format!("paravent-cle-{}-{nom}", std::process::id()));
            std::fs::write(&chemin, contenu).unwrap();
            std::fs::set_permissions(&chemin, std::fs::Permissions::from_mode(mode)).unwrap();
            chemin
        }

        #[test]
        fn lit_une_cle_reservee_a_son_proprietaire() {
            let chemin = fichier_de_cle("ok", &[9; 32], 0o600);
            let lue = Cle::lire_fichier(&chemin);
            std::fs::remove_file(&chemin).unwrap();
            assert!(lue.is_ok());
        }

        #[test]
        fn refuse_une_cle_lisible_par_d_autres() {
            for (nom, mode) in [("groupe", 0o640), ("tous", 0o604), ("ecriture", 0o620)] {
                let chemin = fichier_de_cle(nom, &[9; 32], mode);
                let lue = Cle::lire_fichier(&chemin);
                std::fs::remove_file(&chemin).unwrap();
                assert_eq!(lue.unwrap_err(), ErreurCle::TropOuverte, "mode {mode:o}");
            }
        }

        #[test]
        fn refuse_une_cle_absente_courte_ou_qui_n_est_pas_un_fichier() {
            assert_eq!(
                Cle::lire_fichier(Path::new("/nulle/part/cle")).unwrap_err(),
                ErreurCle::Inaccessible
            );
            assert_eq!(
                Cle::lire_fichier(&std::env::temp_dir()).unwrap_err(),
                ErreurCle::PasUnFichier
            );
            let chemin = fichier_de_cle("courte", &[9; 8], 0o600);
            let lue = Cle::lire_fichier(&chemin);
            std::fs::remove_file(&chemin).unwrap();
            assert_eq!(lue.unwrap_err(), ErreurCle::TropCourte);
        }
    }
}
