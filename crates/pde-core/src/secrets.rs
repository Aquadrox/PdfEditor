//! Protection des données sensibles par le système (port) : clés privées
//! des identités de signature, plus tard clés API. L'adaptateur réel est
//! dans `pde-platform` (DPAPI sous Windows, trousseau de la session sous
//! Linux) ; les tests utilisent un faux.

/// Chiffre des données pour l'utilisateur courant, sur cet ordinateur.
pub trait DataProtector: Send + Sync {
    /// Données chiffrées, lisibles seulement par ce compte sur ce PC.
    fn protect(&self, data: &[u8]) -> Result<Vec<u8>, String>;
    /// Données d'origine d'un bloc produit par [`DataProtector::protect`].
    fn unprotect(&self, data: &[u8]) -> Result<Vec<u8>, String>;
}

/// Trousseau du système : secrets courts (clés API), rangés par nom.
pub trait SecretStore: Send + Sync {
    fn load(&self, name: &str) -> Option<String>;
    fn store(&self, name: &str, value: &str) -> Result<(), String>;
    /// Supprime le secret ; sans effet s'il n'existe pas.
    fn delete(&self, name: &str) -> Result<(), String>;
}
