# Fonctions

Chaque fonction a un identifiant (`E-03`…) repris par la roadmap, les PR et
les tests. Priorité : **M** indispensable à la v1.0 · **S** souhaitable · **P** plus tard.

## V — Visionneuse

| Id | Fonction | Prio |
|---|---|---|
| V-01 | Ouvrir un PDF : dialogue, glisser-déposer, fichiers récents, association `.pdf`, ligne de commande | M |
| V-02 | PDF protégé par mot de passe (ouverture) ; affichage des restrictions du document | M |
| V-03 | Rendu fidèle des pages ; zoom (molette, pincement, ajusté largeur / page, valeurs fixes), défilement continu ou page par page | M |
| V-04 | Miniatures des pages, signets (plan du document), aller à la page | M |
| V-05 | Sélection, copie et recherche de texte (occurrences surlignées, suivante / précédente) | M |
| V-06 | Plusieurs documents ouverts (onglets) | S |
| V-07 | Mode présentation, mode sombre du document (inversion à l'affichage seulement) | P |
| V-08 | Palette de commandes (Ctrl+K) : toutes les actions de l'éditeur par leur nom, avec leur raccourci ; « Comment faire ? » trouve l'outil et le montre dans la barre, sans IA | S |

## E — Édition

Toute modification est annulable (Ctrl+Z / Ctrl+Y, historique illimité dans la session).

| Id | Fonction | Prio |
|---|---|---|
| E-01 | Ajouter une zone de texte : police, taille, couleur, gras / italique, alignement, interligne ; polices embarquées (sous-ensemble) | M |
| E-02 | Ajouter une image (PNG, JPEG, WebP ; presse-papier) : déplacer, redimensionner (proportions verrouillables), pivoter, opacité | M |
| E-03 | Sélectionner, déplacer, aligner, dupliquer, supprimer les éléments ajoutés ; magnétisme sur une grille et entre éléments | M |
| E-04 | Annotations : surlignage, souligné, barré, note, dessin à main levée, formes (trait, flèche, rectangle, ellipse) | M |
| E-05 | Modifier le texte existant du PDF (bloc par bloc), en gardant la police si elle contient les caractères, sinon police de remplacement signalée | M |
| E-06 | Déplacer, remplacer, supprimer une image existante du PDF | S |
| E-07 | Caviardage : suppression réelle du texte et des images sous la zone (pas un simple rectangle noir), avec confirmation | M |
| E-08 | Remplir un formulaire PDF (AcroForm) ; aplatir le formulaire | S |
| E-09 | Tampons (« Approuvé », « Brouillon », date…) et filigranes | S |
| E-10 | En-têtes, pieds de page, numérotation des pages | P |
| E-11 | Métadonnées (titre, auteur, sujet, mots-clés) | S |

## P — Pages

| Id | Fonction | Prio |
|---|---|---|
| P-01 | Insérer une page blanche, supprimer, dupliquer, pivoter | M |
| P-02 | Réordonner par glisser-déposer dans les miniatures | M |
| P-03 | Fusionner des PDF, insérer les pages d'un autre PDF, extraire des pages dans un nouveau PDF | M |
| P-04 | Recadrer, redimensionner (format A4, Lettre…) | S |
| P-05 | Images → PDF (assembler des photos ou des scans) | S |
| P-06 | Compression / optimisation de la taille du fichier | P |

## F — Fichiers

| Id | Fonction | Prio |
|---|---|---|
| F-01 | Enregistrer, Enregistrer sous ; le fichier d'origine n'est jamais écrasé avant que le nouveau soit écrit et relu | M |
| F-02 | Sauvegarde automatique du travail en cours et récupération après plantage | M |
| F-03 | Enregistrement incrémental quand le document contient des signatures (les signatures existantes restent valides tant que les modifications sont autorisées) | M |
| F-04 | Exporter une page en image, exporter le texte | S |
| F-05 | Protéger par mot de passe (chiffrement AES-256) et définir les permissions | S |
| F-06 | Impression (via le système) | S |

## S — Signature

| Id | Fonction | Prio |
|---|---|---|
| S-01 | Identité de signature : nom, e-mail, organisation ; certificat **auto-signé** généré par l'application (clé privée protégée par le système) | M |
| S-02 | Importer un certificat PKCS#12 (`.p12` / `.pfx`) ; mot de passe demandé, ou mémorisé dans le trousseau si l'utilisateur le choisit | M |
| S-03 | Apparence de la signature : signature manuscrite dessinée, importée (image) ou tapée ; nom, date, motif, lieu ; modèles enregistrés | M |
| S-04 | Placer un champ de signature, signer un champ existant | M |
| S-05 | Signature numérique **PAdES** (CMS détaché, SHA-256, RSA ou ECDSA) | M |
| S-06 | **Verrouiller** : signature de certification (DocMDP) — aucune modification permise, ou seulement le remplissage de formulaires et d'autres signatures ; l'application passe le document en lecture seule | M |
| S-07 | Vérifier les signatures d'un document : intégrité, modifications après signature, certificat, horodatage ; panneau « Signatures » | M |
| S-08 | Horodatage RFC 3161 par un serveur au choix (PAdES B-T) | S |
| S-09 | Certificats du magasin Windows et cartes / clés USB (PKCS#11) | P |
| S-10 | Validation à long terme (PAdES B-LT / B-LTA : OCSP, CRL, horodatage de document) | P |

## C — Cycle de signature

Plusieurs personnes signent le même document, dans un ordre défini, sans
serveur obligatoire : le plan du cycle voyage **dans le PDF**.

| Id | Fonction | Prio |
|---|---|---|
| C-01 | Créer un cycle : signataires (nom, e-mail, rôle), ordre séquentiel ou parallèle, champs de signature placés et attribués à chacun, échéance, message | M |
| C-02 | Le plan est embarqué dans le PDF (fichier joint signé par l'initiateur) : chaque destinataire ouvre le document dans l'application et voit « À vous de signer » et où | M |
| C-03 | Envoi au signataire suivant : courriel préparé dans la messagerie par défaut avec le PDF joint (ou enregistrement dans un dossier partagé) | M |
| C-04 | Suivi : qui a signé, quand, qui est attendu ; refus motivé possible | M |
| C-05 | Journal d'audit du cycle (événements chaînés par empreinte, vérifiable), page de synthèse ajoutable en fin de document | S |
| C-06 | Clôture : le dernier signataire (ou l'initiateur) verrouille le document (S-06) | M |
| C-07 | Modèles de cycles réutilisables | S |
| C-08 | Transports supplémentaires : dossier synchronisé, serveur auto-hébergé, connecteurs e-mail (IMAP / SMTP) | P |

## IA — Intelligence artificielle

Portée de chaque action IA : **sélection**, **pages choisies** ou **document entier**.
Le document est toujours traité comme une donnée : les consignes qu'il contient ne sont jamais exécutées.
Aucune modification n'est appliquée sans aperçu ni accord de l'utilisateur.

| Id | Fonction | Prio |
|---|---|---|
| IA-01 | Fournisseur au choix (API compatible OpenAI) : préréglages DeepSeek, OpenAI, Mistral, GLM, OpenRouter, Ollama / LM Studio local, adresse libre ; profils ; profil de secours ; test des réglages ; liste des modèles du serveur | M |
| IA-02 | Clé API dans le trousseau du système, jamais sur le disque en clair ; jamais envoyée en clair hors du PC | M |
| IA-03 | Estimation du volume et du coût avant l'envoi ; confirmation au-delà d'un seuil réglable ; suivi de la consommation (jour, mois, total) ; plafond mensuel optionnel | M |
| IA-04 | **Analyse** : résumé, points clés, structure, entités (personnes, dates, montants), risques et clauses (contrats), avec renvoi aux pages | M |
| IA-05 | **Questions sur le document** (discussion), réponses citant les pages, clic sur une citation → page | M |
| IA-06 | **Traduction** partielle ou totale vers une langue au choix, mise en page conservée (texte remplacé dans ses blocs, taille ajustée) | M |
| IA-07 | Modes de traduction : nouveau document traduit, document bilingue (pages alternées), texte seul dans le panneau | S |
| IA-08 | Glossaire de traduction (termes imposés ou à ne pas traduire) | S |
| IA-09 | **Modification** par consigne (« reformule plus formel », « corrige les fautes », « mets à jour les dates »…) : propositions affichées en différences, acceptées ou refusées bloc par bloc, annulables | M |
| IA-10 | Anonymisation assistée : l'IA propose les zones à caviarder (noms, adresses, IBAN…), l'utilisateur valide, le caviardage E-07 s'applique | S |
| IA-11 | Prompts système éditables par action, prompt intégré en référence, test sur exemple | M |
| IA-12 | Annulation d'une action en cours ; réponses affichées au fil de l'eau | S |
| IA-13 | Pages scannées : reconnaissance du texte (OCR local, ou modèle IA avec vision) | P |
| IA-14 | Remplissage assisté de formulaire | P |
| IA-15 | **Assistant d'actions** : sur une sélection (zone, texte, pages) ou le document, une demande en langage courant, écrite ou dictée (« caviarde-moi ça », « masque ce numéro », « surligne toutes les dates », « tourne les pages scannées »). L'IA répond par des **actions proposées**, tirées du catalogue de l'éditeur, affichées en aperçu sur la page et appliquées seulement sur accord (une seule annulation) ; ou elle explique comment faire et montre l'outil dans la barre. Elle distingue masquer (un aplat, le contenu reste dans le fichier) et caviarder (suppression réelle) et le dit | M |
| IA-16 | **Recettes** (compétences sur mesure) : une consigne enregistrée avec les seules actions qu'elle peut proposer (« Anonymiser selon le RGPD », « Préparer pour signature », « Relire les montants ») ; recettes intégrées et personnelles, modifiables, exportables en fichier pour les partager | S |
| IA-17 | **Serveur MCP local**, désactivé par défaut : le même catalogue d'actions offert aux assistants externes (Claude Desktop, autres clients MCP) sur le document ouvert ; lecture seule par défaut, chaque modification confirmée dans PdfEditor | P |

## A — Android (v0.8)

Même application, sur téléphone et tablette Android.

| Id | Fonction | Prio |
|---|---|---|
| A-01 | Interface tactile : panneaux en tiroirs, pincer pour zoomer, sélection de texte et placement d'éléments au doigt | S |
| A-02 | Ouvrir depuis le sélecteur de documents, « Ouvrir avec » et « Partager vers PdfEditor » ; enregistrer ou partager le résultat | S |
| A-03 | Lecture, recherche, annotations, signature et actions IA comme sur PC ; clés et certificats dans le Keystore Android | S |
| A-04 | Mises à jour proposées par l'application (APK signé, vérifié avant installation) | S |
| A-05 | Réglages partagés avec le PC dans leur format (`config.toml`), adaptés au téléphone | P |

## R — Réglages

Comme VoiceToText, **tout est réglable** dans la fenêtre Réglages, appliqué à
chaud, enregistré dans `config.toml` (lisible et éditable à la main, validé au
chargement ; une valeur invalide est signalée et remplacée par sa valeur par défaut).

| Id | Rubrique | Contenu |
|---|---|---|
| R-01 | Général | Langue de l'interface, thème (clair / sombre / système), fichiers récents, ouverture au démarrage, unité (mm / pt / pouce) |
| R-02 | Affichage | Zoom par défaut, mode de défilement, lissage, couleur de fond, qualité de rendu, cache |
| R-03 | Édition | Police, taille et couleur par défaut, grille et magnétisme, couleurs d'annotation, nom d'auteur des annotations |
| R-04 | Fichiers | Sauvegarde automatique (intervalle), dossier par défaut, suffixe des copies, comportement à l'enregistrement d'un document signé |
| R-05 | Signature | Identités, certificats, apparences, verrouillage par défaut, serveur d'horodatage, algorithme |
| R-06 | Cycle | Message par défaut, transport, rappels, modèles de cycles |
| R-07 | IA | Profils (fournisseur, modèle, délai, tarif, paramètres propres), clés, profil de secours, prompts, langues favorites, glossaires, seuil de confirmation, plafond de coût, confirmation avant tout envoi |
| R-08 | Raccourcis | Tous les raccourcis clavier, modifiables |
| R-09 | Mises à jour et diagnostic | Vérification des mises à jour, niveau de journal, ouverture du dossier des journaux |
