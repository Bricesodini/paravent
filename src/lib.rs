//! `paravent` : un filtre entre un agent et des données personnelles.
//!
//! L'agent agit sur des références, des comptes et des états ; les valeurs personnelles ne lui
//! parviennent jamais. Vocabulaire et garanties : `docs/cadrage.md`.

pub mod action;
pub mod banc;
pub mod declaration;
pub mod filtre;
pub mod modele;
pub mod personnel;
pub mod reference;

pub use declaration::{Classe, Declaration, ErreurDeclaration};
pub use personnel::Personnel;
pub use reference::{Cle, ErreurCle, ErreurReference, Fabrique, Reference};
