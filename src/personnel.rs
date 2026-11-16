//! Une valeur personnelle, qu'on ne peut ni afficher ni journaliser par mégarde.

use std::fmt;

/// Porte une valeur personnelle.
///
/// Le type n'implémente ni `Display` ni `Serialize`, et son `Debug` est masqué : une valeur ne
/// part pas dans une sortie ou un journal sans un appel explicite à [`Personnel::exposer`], qui
/// se cherche dans le code.
///
/// `Debug` ne montre rien :
///
/// ```
/// let p = paravent::Personnel::new("Camille");
/// assert_eq!(format!("{p:?}"), "Personnel(masqué)");
/// ```
///
/// L'affichage ne compile pas :
///
/// ```compile_fail
/// let p = paravent::Personnel::new("Camille");
/// println!("{}", p);
/// ```
///
/// La sérialisation non plus :
///
/// ```compile_fail
/// let p = paravent::Personnel::new("Camille");
/// let _ = serde_json::to_string(&p);
/// ```
#[derive(Clone)]
pub struct Personnel(String);

impl Personnel {
    pub fn new(valeur: impl Into<String>) -> Self {
        Self(valeur.into())
    }

    /// Vrai si la valeur contient autre chose que des blancs.
    pub fn est_renseigne(&self) -> bool {
        !self.0.trim().is_empty()
    }

    /// Nombre de caractères.
    pub fn longueur(&self) -> usize {
        self.0.chars().count()
    }

    /// Rend la valeur en clair. À n'appeler que du côté de la source, au moment d'agir.
    pub fn exposer(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Personnel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Personnel(masqué)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_debug_ne_montre_pas_la_valeur() {
        let p = Personnel::new("Camille Roche");
        assert!(!format!("{p:?}").contains("Camille"));
        assert!(!format!("{:#?}", Some(&p)).contains("Camille"));
    }

    #[test]
    fn presence_et_longueur_sans_la_valeur() {
        assert!(Personnel::new("Élodie").est_renseigne());
        assert_eq!(Personnel::new("Élodie").longueur(), 6);
        assert!(!Personnel::new("  \n").est_renseigne());
    }

    #[test]
    fn la_valeur_ne_sort_que_sur_demande_explicite() {
        assert_eq!(Personnel::new("Camille").exposer(), "Camille");
    }
}
