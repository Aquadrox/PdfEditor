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

## v0.1 — Visionneuse ✅

Première version publiée : v0.0.5 (16/11/2026), installeur Windows signé et mises à jour automatiques.

- ✅ `cargo-deny` dans la CI (`deny.toml`) : aucune vulnérabilité connue, licences compatibles avec MIT, sources crates.io uniquement
- ✅ `pde-pdf` : PDFium 8125 figé et vérifié (SHA-256, `scripts/fetch-pdfium.sh`) ; thread moteur dédié ; ouverture, mot de passe, rendu, texte avec positions, plan (V-01, V-02, V-03)
- ✅ PDF de référence générés (`crates/pde-pdf/tests/fixtures/make_fixtures.py`) : pages tournées, plan, chiffrement AES-256, fichiers invalides
- ➡️ Tests de rendu comparés à des images de référence : reportés en v0.2 (aujourd'hui : contrôles de pixels ciblés)
- ✅ `pde-app` : visionneuse testable sans interface (faux moteur), cache des pages limité en mémoire, PNG
- ✅ `apps/desktop` : fenêtre principale selon la maquette, protocole `pde://`, zoom, défilement continu, miniatures, plan, aller à la page, mot de passe, glisser-déposer, ouverture en argument (V-01 à V-04)
- ✅ Tests de l'interface : modules JS (`node --test`) et parcours dans Chromium avec Tauri simulé (Playwright)
- ✅ Couche texte : sélection, copie, recherche insensible à la casse et aux accents, positions correctes sur les pages tournées (V-05)
- ✅ `pde-core::settings` (`config.toml` tolérant, valeurs bornées, raccourcis vérifiés) + page Réglages : Général, Affichage, Raccourcis clavier, Diagnostic, appliqués à chaud (R-01, R-02, R-08, R-09)
- ✅ `pde-platform` : dossiers de l'application (`PDE_HOME` pour les tests) ; le trousseau arrive avec la signature (v0.4)
- ✅ Documents récents sur l'accueil, rouvrir le dernier document au démarrage (V-01)
- ✅ Journal repris de VoiceToText (purge automatique, niveau réglable, paniques notées), sans nom ni contenu de document
- ✅ Installeur Windows (NSIS, PDFium inclus) construit par la CI ; version vérifiée contre le tag ; `CHANGELOG.md` contrôlé (`changelog.py`) ; `release.sh` (repris de VoiceToText)
- ✅ Mises à jour automatiques (repris de VoiceToText) : vérification au démarrage puis toutes les 6 h (réglable), bandeau avec nouveautés, installation sur clic après vérification de la signature minisign et de la version signée ; release publiée par la CI avec `latest.json`
- ✅ Clé de signature des mises à jour : générée par le propriétaire (identifiant `D6F1230550475C66`), clé publique dans `tauri.conf.json`, clé privée dans les secrets GitHub

## v0.2 — Édition : ajouts 🚧

- ✅ Garde-fou Android dans la CI : `pde-core`, `pde-app`, `pde-pdf` et `pde-platform` compilent pour Android (aarch64), pour ne jamais fermer la porte à la v1.1
- ✅ Tests de rendu comparés à des images de référence (`crates/pde-pdf/tests/fixtures/expected/`, `PDE_UPDATE_EXPECTED=1` pour les régénérer)
- ✅ Modèle d'édition (`pde-core::edit`) : éléments ajoutés, historique annuler / rétablir, suivi « modifié », coupure des lignes ; le moteur repart du PDF d'origine et repose les éléments à chaque changement
- ✅ Sauvegarde automatique et récupération après plantage (F-02) : ajouts (textes, images) mis en lieu sûr à l'intervalle réglé, jamais le PDF ni un document protégé par mot de passe ; au lancement suivant, Récupérer / Plus tard / Ignorer ; reprise refusée si le fichier a changé (empreinte SHA-256)
- ✅ Mode Éditer : barre d'outils, couche d'édition sur les pages, panneau Propriétés (maquette `Main.dc.html`), annuler / rétablir (Ctrl+Z / Ctrl+Y), pastille « modifié », raccourcis réglables ; tests de parcours Playwright
- 🚧 Zones de texte (E-01) : création par un clic et saisie directe sur la page, police standard, taille, gras, italique, couleur, alignement, déplacement et largeur à la souris ou en mm, dupliquer, supprimer ; reste les polices embarquées pour les autres alphabets (Noto)
- ✅ Images (E-02) : PNG, JPEG, WebP par le bouton Image, le presse-papiers (Ctrl+V) ou un glisser-déposer ; JPEG inclus sans recompression, transparence gardée, orientation EXIF appliquée ; coins avec proportions verrouillables, rotation par quarts de tour, opacité ; tests de rendu (image de référence `images.png`)
- 🚧 Sélection, déplacement, alignement, magnétisme (E-03) : sélection, déplacement (souris, flèches), dupliquer, supprimer faits ; reste sélection au clavier (Tab), alignement, magnétisme
- ✅ Annotations et formes (E-04) : surligner, souligner, barrer le texte sélectionné ; dessin à main levée ; rectangle, ellipse, trait, flèche (Maj : carré, cercle, 45°) ; couleur, épaisseur, remplissage ; poignées ; tracés dans le contenu de la page (même rendu partout), image de référence `annotations.png` ; notes = vraies annotations PDF « Texte » (commentaire, auteur, icône) lisibles dans les autres lecteurs
- ✅ Enregistrer / Enregistrer sous (F-01) : contenu relu avant d'écrire, remplacement atomique, mot de passe conservé ; question avant de fermer le document ou la fenêtre, ou d'en ouvrir un autre, avec des modifications
- 🚧 Réglages Édition et Fichiers (R-03, R-04) : nom d'auteur des notes, police, taille et couleur des nouvelles zones de texte, couleur de surlignage, intervalle de la sauvegarde automatique faits ; restent grille et magnétisme (avec E-03), dossier par défaut, suffixe des copies

## v0.3 — Contenu existant et pages

- ⬜ Modifier le texte existant, police de remplacement (E-05)
- ⬜ Images existantes (E-06)
- ⬜ Caviardage réel (E-07)
- ⬜ Pages : insérer, supprimer, pivoter, réordonner, fusionner, extraire (P-01, P-02, P-03)
- ⬜ Formulaires, métadonnées (E-08, E-11)
- ⬜ Tests de parcours de l'interface (Playwright)

## v0.4 — Signature et verrouillage

- ⬜ `pde-platform` : trousseau du système (repris de VoiceToText) pour les mots de passe de certificats

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

- ⬜ Version Linux (AppImage)
- ⬜ Accessibilité (clavier complet, lecteurs d'écran, contrastes), performances sur gros documents
- ⬜ Manuel d'utilisation
- ⬜ Fonctions S restantes : onglets (V-06), tampons (E-09), recadrage (P-04), images → PDF (P-05), export (F-04), mot de passe (F-05), impression (F-06)

## v1.0 — Version stable

- ⬜ Toutes les fonctions **M** livrées, testées, documentées

## v1.1 — Android

Même code Rust et même interface (Tauri 2 en version mobile), adaptée au
tactile. Fonctions A-01 à A-05 de `FONCTIONS.md`.

- ⬜ Application Android Tauri 2 (arm64), PDFium pour Android figé et vérifié (SHA-256)
- ⬜ Interface tactile : panneaux en tiroirs, barre d'outils compacte, pincer pour zoomer, sélection au doigt (A-01)
- ⬜ Fichiers : sélecteur de documents Android, « Ouvrir avec » et « Partager vers PdfEditor », enregistrement (A-02)
- ⬜ Lecture, recherche, annotations, signature et IA ; secrets dans le Keystore Android (A-03)
- ⬜ Mises à jour : APK signé (clé du propriétaire dans les secrets GitHub), manifeste `latest-android.json` avec taille et SHA-256, comme VoiceToText (A-04)
- ⬜ CI : APK de test à chaque PR, APK signé sur un tag ; tests sur émulateur des parcours principaux (A-05)

## Plus tard

OCR (IA-13), remplissage assisté (IA-14), PKCS#11 et magasin Windows (S-09),
PAdES B-LT / B-LTA (S-10), transports de cycle (C-08), en-têtes et pieds de
page (E-10), compression (P-06), mode présentation (V-07), macOS, comparaison
de deux versions d'un document.
