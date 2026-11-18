//! Erreurs de la signature, en mots d'utilisateur quand elles remontent
//! jusqu'à l'interface.

/// Ce qui empêche de créer une identité, de signer ou de lire une signature.
#[derive(Debug, thiserror::Error)]
pub enum SignError {
    /// Le PDF ne peut pas être lu (structure invalide).
    #[error("le document n'est pas un PDF lisible : {0}")]
    Pdf(String),
    /// Document protégé : mot de passe absent ou incorrect.
    #[error("document protégé par mot de passe : mot de passe absent ou incorrect")]
    Encrypted,
    /// Verrouiller exige d'être la première signature du document (DocMDP).
    #[error("un document déjà signé ne peut plus être verrouillé par une nouvelle signature")]
    AlreadySigned,
    /// Le document est verrouillé par une certification sans modification permise.
    #[error("le document est verrouillé : il ne peut plus recevoir de signature")]
    Locked,
    /// La page ou le champ demandé n'existe pas.
    #[error("{0}")]
    Target(String),
    /// Mot de passe du fichier PKCS#12 refusé.
    #[error("mot de passe du certificat incorrect")]
    WrongPassword,
    /// Le certificat ou la clé ne convient pas (algorithme, contenu).
    #[error("certificat inutilisable : {0}")]
    Certificate(String),
    /// La signature calculée dépasse la place réservée dans le fichier.
    #[error("la signature dépasse la place réservée dans le fichier")]
    TooLarge,
    /// Erreur d'encodage ASN.1 (ne devrait pas arriver).
    #[error("encodage de la signature impossible : {0}")]
    Encoding(String),
    /// Horodatage impossible (serveur injoignable, refus, réponse invalide).
    #[error("horodatage impossible : {0}")]
    Timestamp(String),
    /// Aléa du système indisponible.
    #[error("générateur aléatoire du système indisponible")]
    Random,
}

impl From<der::Error> for SignError {
    fn from(error: der::Error) -> Self {
        SignError::Encoding(error.to_string())
    }
}

impl From<std::io::Error> for SignError {
    fn from(error: std::io::Error) -> Self {
        SignError::Encoding(error.to_string())
    }
}

impl From<lopdf::Error> for SignError {
    fn from(error: lopdf::Error) -> Self {
        SignError::Pdf(error.to_string())
    }
}

/// Résultat des fonctions de ce crate.
pub type Result<T> = std::result::Result<T, SignError>;
