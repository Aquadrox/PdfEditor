# Roadmap et avancement

Légende : ✅ fait · 🚧 en cours · ⬜ à faire. Les identifiants renvoient à `FONCTIONS.md`.

Chaque version est livrée par une ou plusieurs PR, chacune avec ses tests ;
une version n'est terminée que lorsque la CI est verte et que la fonction a
été essayée sur le PC (Windows).

## v0.0 — Fondations ✅

- ✅ Fonctions (`FONCTIONS.md`), architecture (`ARCHITECTURE.md`), design (`DESIGN.md`), règles de contribution
- ✅ Workspace Cargo, règles de lint communes, crate `pde-core` (unités, géométrie) avec tests
- ✅ CI GitHub Actions : `linux` (format, clippy, tests), `windows`, `secrets` (gitleaks)
- ✅ Branche `main` créée et protégée (PR obligatoire, jobs `linux`, `windows`, `secrets` requis)
- ✅ Maquette des écrans principaux validée (`docs/maquette/`, voir `DESIGN.md`)

## v0.1 — Visionneuse 🚧

- ⬜ `cargo-deny` dans la CI (licences compatibles, vulnérabilités connues)
- ✅ `pde-pdf` : PDFium 8125 figé et vérifié (SHA-256, `scripts/fetch-pdfium.sh`) ; thread moteur dédié ; ouverture, mot de passe, rendu, texte avec positions, plan (V-01, V-02, V-03)
- ✅ PDF de référence générés (`crates/pde-pdf/tests/fixtures/make_fixtures.py`) : pages tournées, plan, chiffrement AES-256, fichiers invalides
- ⬜ Tests de rendu comparés à des images de référence (aujourd'hui : contrôles de pixels ciblés)
- ⬜ `apps/desktop` : fenêtre principale selon `DESIGN.md`, protocole `pde://`, zoom, défilement, miniatures, plan (V-03, V-04)
- ⬜ Couche texte : sélection, copie, recherche (V-05)
- ⬜ `pde-core::settings` + page Réglages (Général, Affichage, Raccourcis) ; `pde-platform` (dossiers, trousseau repris de VoiceToText)
- ⬜ Journaux, rapport de plantage
- ⬜ Installeur Windows construit par la CI ; version vérifiée contre le tag ; `CHANGELOG.md` contrôlé (`changelog.py`, repris de VoiceToText)

## v0.2 — Édition : ajouts

- ⬜ Modèle d'opérations et historique annuler / rétablir (`pde-core`), sauvegarde automatique et récupération (F-02)
- ⬜ Zones de texte, polices embarquées (E-01)
- ⬜ Images (E-02)
- ⬜ Sélection, déplacement, alignement, magnétisme (E-03)
- ⬜ Annotations et formes (E-04)
- ⬜ Enregistrer / Enregistrer sous, écriture sûre (F-01)
- ⬜ Réglages Édition et Fichiers (R-03, R-04)

## v0.3 — Contenu existant et pages

- ⬜ Modifier le texte existant, police de remplacement (E-05)
- ⬜ Images existantes (E-06)
- ⬜ Caviardage réel (E-07)
- ⬜ Pages : insérer, supprimer, pivoter, réordonner, fusionner, extraire (P-01, P-02, P-03)
- ⬜ Formulaires, métadonnées (E-08, E-11)
- ⬜ Tests de parcours de l'interface (Playwright)

## v0.4 — Signature et verrouillage

- ⬜ `pde-sign` : écriture incrémentale, signature PAdES B-B (S-05), vérifiée par `pdfsig` / `qpdf` dans la CI
- ⬜ Identités : certificat auto-signé, import PKCS#12 (S-01, S-02)
- ⬜ Apparence : manuscrite dessinée / importée / tapée (S-03), champs de signature (S-04)
- ⬜ Verrouillage DocMDP et passage en lecture seule (S-06)
- ⬜ Panneau Signatures : vérification (S-07)
- ⬜ Enregistrement incrémental des documents signés (F-03)
- ⬜ Horodatage RFC 3161 (S-08)

## v0.5 — IA : socle et analyse

- ⬜ `pde-ai` : client repris de VoiceToText, découpé et testé ; trait `ChatModel` et faux modèle de test ; réponses en flux
- ⬜ Réglages IA : préréglages, profils, clé dans le trousseau, test des réglages, profil de secours, prompts éditables (IA-01, IA-02, IA-11, R-07)
- ⬜ Estimation du coût, confirmation, consommation, plafond (IA-03)
- ⬜ Panneau IA : analyse sur sélection / pages / document (IA-04)
- ⬜ Questions sur le document avec citations de pages (IA-05)

## v0.6 — IA : traduction

- ⬜ Traduction par blocs, mise en page conservée, ajustement de la taille (IA-06)
- ⬜ Modes : document traduit, bilingue, texte seul (IA-07)
- ⬜ Glossaires (IA-08)

## v0.7 — IA : modification

- ⬜ Modification par consigne, différences, acceptation bloc par bloc, annulable (IA-09)
- ⬜ Anonymisation assistée (IA-10)
- ⬜ Annulation d'une action en cours (IA-12)

## v0.8 — Cycle de signature

- ⬜ `pde-workflow` : plan, machine à états, manifeste embarqué, journal d'audit (C-01, C-02, C-05)
- ⬜ Assistant de création du cycle, champs attribués (C-01)
- ⬜ « À vous de signer », envoi au suivant, suivi, refus (C-03, C-04)
- ⬜ Clôture et verrouillage (C-06), modèles de cycles (C-07)

## v0.9 — Finition et distribution

- ⬜ Mises à jour automatiques signées (comme VoiceToText)
- ⬜ Version Linux (AppImage)
- ⬜ Accessibilité (clavier complet, lecteurs d'écran, contrastes), performances sur gros documents
- ⬜ Manuel d'utilisation
- ⬜ Fonctions S restantes : onglets (V-06), tampons (E-09), recadrage (P-04), images → PDF (P-05), export (F-04), mot de passe (F-05), impression (F-06)

## v1.0 — Version stable

- ⬜ Toutes les fonctions **M** livrées, testées, documentées

## Plus tard

OCR (IA-13), remplissage assisté (IA-14), PKCS#11 et magasin Windows (S-09),
PAdES B-LT / B-LTA (S-10), transports de cycle (C-08), en-têtes et pieds de
page (E-10), compression (P-06), mode présentation (V-07), macOS, comparaison
de deux versions d'un document.
