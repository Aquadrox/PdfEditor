# Notes de version

Une section par version, la plus récente en tête. Une ligne `- ` par
changement, en mots d'utilisateur ; `[Windows]`, `[Linux]` ou `[Android]` devant ce
qui ne concerne qu'une plateforme.

## 0.0.29 — 2026-11-18

- Mode IA, « Modifier » et traduction : corrige « aucun texte à modifier (page scannée ?) » sur les documents A4 ordinaires (LibreOffice, Word…), dont le texte n'était pas reconnu.
- Mode IA, « Action » : une demande qui change le texte (« Renomme X en Y », « corrige les fautes ») est confiée à « Modifier », qui propose chaque ligne modifiée à accepter ou refuser.
- Menu principal : il s'ouvre sous son bouton et entièrement à l'écran ; « Ouvrir », « Enregistrer » et « Fermer » étaient cachés en haut.
- Mode IA : le texte des réponses se sélectionne à la souris pour le copier-coller, et le bouton « Copier » copie toute la réponse.

## 0.0.28 — 2026-11-18

- [Android] Ouvrir un PDF depuis le sélecteur de documents du téléphone ; « Enregistrer » le réécrit à son emplacement d'origine, « Enregistrer sous » propose de choisir où.
- [Android] PdfEditor apparaît dans « Ouvrir avec » et « Partager » pour les PDF : le document s'ouvre aussitôt, même si l'application était déjà lancée.
- Mode IA : le bas du panneau (type de demande, recette, champ) est enfin utilisable, sur plusieurs lignes ; le panneau s'élargit en tirant son bord gauche (ou avec les flèches), et seule la liste des réponses défile.
- Mode IA : chaque réponse (résumé, question…) peut être supprimée du panneau avec son bouton ×.
- Réglages IA : « Tester » affiche les modèles du serveur en boutons, comme VoiceToText ; un clic choisit le modèle.

## 0.0.27 — 2026-11-18

- [Android] Première version de test de l'application Android (téléphones ARM64), construite à chaque modification : même moteur et même interface que sur PC. L'ouverture des documents depuis le téléphone, l'interface tactile et les mises à jour arrivent dans les versions suivantes.

## 0.0.26 — 2026-11-18

- Modifier le texte avec l'IA : en mode IA, choisissez « Modifier » et donnez une consigne (« corrige les fautes », « plus formel », « mets à jour les dates ») ; chaque ligne changée est proposée en différences, encadrée sur la page.
- Cochez les lignes à garder, puis « Appliquer » : rien n'est modifié sans votre accord, et une seule annulation (Ctrl+Z) défait l'ensemble.
- Nouveau bouton « Anonymiser » : l'IA repère les données personnelles (noms, coordonnées, IBAN…) et propose de les caviarder, en aperçu.
- Traduire, modifier le texte avec l'IA et anonymiser se trouvent aussi par la palette de commandes (Ctrl+K).

## 0.0.25 — 2026-11-18

- Traduction par l'IA vers la langue de votre choix, sur une sélection, des pages ou tout le document : chaque ligne est remplacée à sa place, avec sa police quand c'est possible.
- Trois modes : remplacer le texte (« Enregistrer sous » garde l'original), document bilingue (chaque page suivie de sa traduction) ou texte seul dans le panneau.
- Une traduction plus longue que l'original est réduite jusqu'à un corps minimal réglable ; les lignes qui restent trop longues, ou que le modèle n'a pas rendues, sont signalées. Annuler (Ctrl+Z) défait toute la traduction.
- Glossaire dans les Réglages (rubrique IA) : imposez la traduction d'un terme, ou gardez-le tel quel.

## 0.0.24 — 2026-11-18

- Recettes de l'assistant : une consigne enregistrée avec les seules actions qu'elle peut proposer ; en mode IA, en « Action », choisissez-en une et envoyez, avec ou sans précisions.
- Trois recettes intégrées : « Anonymiser selon le RGPD », « Préparer pour signature » et « Relire les montants ».
- Nouvelle rubrique « Recettes » des Réglages : créez les vôtres, dupliquez une recette intégrée pour l'adapter, exportez-les en fichier et importez celles qu'on vous partage.

## 0.0.23 — 2026-11-18

- Assistant d'actions : en mode IA, choisissez « Action » et demandez en mots courants (« caviarde le nom du client », « surligne les dates », « tourne la page 2 ») ; l'IA propose des actions, montrées en aperçu sur les pages.
- Rien n'est appliqué sans votre accord : décochez ce que vous ne voulez pas, puis « Appliquer » ; une seule annulation (Ctrl+Z) défait toutes les actions appliquées.
- Le texte visé est cherché dans le document : une proposition introuvable ou hors de la portée choisie est écartée, avec la raison.
- L'assistant distingue caviarder (suppression réelle, à l'enregistrement) et masquer, qu'il ne propose pas ; avec un modèle qui ne sait pas proposer d'actions, il explique et « Comment faire ? » montre l'outil.

## 0.0.22 — 2026-11-18

- Palette de commandes (Ctrl+K) : toutes les actions de l'éditeur par leur nom, avec leur raccourci ; Entrée exécute l'action, dans le bon mode.
- « Comment faire ? » (F1) : décrivez ce que vous voulez faire, par exemple « tourner une page », et PdfEditor vous montre l'outil dans la barre, avec une explication.
- Les deux sont aussi dans le menu, et leurs raccourcis se changent dans les Réglages.

## 0.0.21 — 2026-11-18

- Nouveau mode IA : résumé, points clés, entités (personnes, dates, montants…) et risques et clauses d'un document, sur une sélection, des pages choisies ou tout le document.
- Posez vos questions sur le document : la réponse s'affiche au fil de l'eau et cite ses pages ; un clic sur une citation affiche la page, et une page inexistante est signalée.
- Fournisseur d'IA au choix (DeepSeek, OpenAI, Mistral, GLM, OpenRouter, Ollama ou LM Studio sur votre PC…), profils et profil de secours ; « Tester » vérifie les réglages et liste les modèles.
- La clé API est rangée dans le trousseau du système, jamais en clair sur le disque, et n'est envoyée qu'en HTTPS.
- Avant d'envoyer du texte à un nouveau fournisseur, ou au-delà d'un coût choisi, PdfEditor montre ce qui part, où, et le coût estimé ; un plafond mensuel peut bloquer les envois.
- La consommation (jour, mois, total) est suivie dans les Réglages ; une action IA en cours peut être annulée.
- Les consignes de chaque action IA sont modifiables ; un long document est lu par parties puis synthétisé.

## 0.0.20 — 2026-11-18

- Un document signé s'enregistre sans perdre ses signatures : vos modifications sont ajoutées à la suite, et le panneau Signatures indique que le document a été complété depuis.
- Signature manuscrite : dessinez-la à la souris ou au stylet, tapez votre nom dans une écriture manuscrite, ou importez la photo de votre signature (le papier blanc devient transparent).
- « Mes signatures » garde vos signatures manuscrites pour les réutiliser d'un clic.
- Horodatage : un serveur d'horodatage atteste l'heure de la signature (seule une empreinte lui est envoyée) ; l'heure attestée apparaît dans le panneau Signatures.
- Nouvelle rubrique Signature dans les Réglages : horodatage par défaut, serveur d'horodatage, verrouillage proposé.
- Un document verrouillé « sauf formulaires et signatures » ne laisse plus que remplir ses champs et le signer.
- Le panneau Signatures dit ce qui a changé depuis chaque signature (champs remplis, nouvelle signature, annotations, contenu des pages…) et signale ce que le verrouillage du document interdisait.
- Les documents protégés par mot de passe se signent aussi ; ils restent protégés.

## 0.0.19 — 2026-11-18

- Nouveau mode Signer : signez vos PDF avec une signature numérique reconnue par les autres lecteurs (Acrobat, navigateurs…).
- Créez votre identité de signature (nom, adresse, organisation) : PdfEditor génère un certificat personnel dont la clé reste protégée sur votre PC ; vous pouvez aussi importer un certificat .p12 ou .pfx.
- Signez sans apparence, dans un cadre que vous tracez sur la page, ou d'un clic dans un champ de signature prévu par le document.
- Verrouillez le document en signant : entièrement, ou en laissant le remplissage des champs et d'autres signatures ; un document verrouillé ne peut plus être modifié.
- Le panneau Signatures vérifie chaque signature (document intact ou complété depuis, certificat), et le sceau de la barre du haut résume l'état du document.
- Le fichier d'origine n'est jamais réécrit : la signature est ajoutée à sa fin, et les signatures déjà présentes restent valides.

## 0.0.18 — 2026-11-17

- Le texte déjà présent dans le PDF se modifie : en mode Éditer, choisissez « Texte du document », cliquez sur une ligne et récrivez-la à sa place.
- La police du document est gardée quand elle contient les caractères saisis ; sinon une police proche la remplace et le panneau Propriétés le signale.
- Vider une ligne du document la supprime ; une annulation la fait revenir.
- Les formulaires PDF se remplissent en mode Lire : champs texte, cases à cocher, boutons radio et listes ; les valeurs sont enregistrées dans le fichier.
- « Aplatir le formulaire » (menu) fige les champs remplis dans la page.
- « Propriétés du document… » (menu) change le titre, l'auteur, le sujet et les mots-clés du PDF.

## 0.0.17 — 2026-11-17

- Les images déjà présentes dans le PDF se modifient en mode Éditer : choisissez-les d'un clic, puis déplacez-les, redimensionnez-les par leurs coins ou supprimez-les.
- « Remplacer… » met une autre image à la place, dans le même cadre ; une seule annulation suffit à revenir en arrière.
- Une image déplacée ou agrandie garde toute sa qualité : elle n'est jamais recompressée.

## 0.0.16 — 2026-11-17

- Caviardage réel : avec l'outil Caviarder, glissez sur du texte ou tracez une zone ; ce qui se trouve dessous (texte, images, annotations) est supprimé du fichier, pas seulement masqué.
- Avant d'enregistrer un document caviardé, PdfEditor demande confirmation : le contenu supprimé ne pourra plus être retrouvé.
- Le reste d'une ligne touchée par une zone garde son aspect, mais devient une image : il n'est plus sélectionnable.

## 0.0.15 — 2026-11-17

- Mode Pages : « Insérer un PDF » ajoute les pages d'un ou plusieurs autres PDF après la sélection ; vous pouvez aussi glisser des PDF sur la grille, ils s'insèrent là où vous les déposez. Pour fusionner, insérez-les à la fin.
- « Extraire » enregistre les pages choisies, avec ce que vous y avez ajouté, dans un nouveau PDF, sans modifier le document.
- La sauvegarde automatique reprend aussi les pages insérées, tant que leur fichier n'a pas changé.

## 0.0.14 — 2026-11-17

- Nouveau mode Pages : toutes les pages du document en grille ; ajoutez une page blanche, dupliquez, faites pivoter ou supprimez les pages choisies (clic, Ctrl+clic, Maj+clic, Ctrl+A).
- Réordonnez les pages en les glissant ; double-cliquez sur une page pour l'ouvrir.
- Ce que vous avez ajouté sur une page (images, dessins, notes) la suit quand elle est déplacée, copiée ou tournée ; chaque opération s'annule (Ctrl+Z).
- Les signets du panneau Plan suivent leurs pages quand vous les réordonnez, et la sauvegarde automatique garde l'ordre des pages.

## 0.0.13 — 2026-11-17

- « Enregistrer sous » propose une copie à côté de l'original, nommée avec un suffixe réglable : « contrat (modifié).pdf ».
- Réglages Fichiers : dossier proposé par les dialogues Ouvrir et Ajouter une image, choisi avec le bouton « Choisir… » ; suffixe des copies.

## 0.0.12 — 2026-11-17

- Sélection de plusieurs éléments : Maj+clic, Ctrl+A pour toute la page, Tab pour passer d'un élément au suivant.
- Les éléments sélectionnés se déplacent, se décalent aux flèches, se dupliquent et se suppriment ensemble ; une seule annulation suffit à revenir en arrière.
- Aligner (bords, centres) et répartir régulièrement plusieurs éléments.
- Magnétisme : en déplaçant un élément, il se cale sur les bords et centres des autres et de la page, avec un repère ; grille réglable dans les réglages Édition, Alt pour s'en passer.
- Correctif : un élément non sélectionné se déplace dès le premier glisser.

## 0.0.11 — 2026-11-17

- Les zones de texte s'écrivent aussi en grec, en cyrillique et dans tout l'alphabet latin (polonais, tchèque, vietnamien…) : la police nécessaire est incluse dans le PDF, seulement quand il le faut.
- Les caractères qui ne peuvent pas encore s'afficher (chinois, arabe…) sont signalés dans le panneau Propriétés.

## 0.0.10 — 2026-11-17

- Notes : cliquez sur la page avec l'outil Note pour y laisser un commentaire ; c'est une vraie note PDF, avec votre nom, que les autres lecteurs affichent aussi.
- Nouvelle rubrique Édition dans les réglages : votre nom d'auteur, la police, la taille et la couleur des nouvelles zones de texte, et la couleur de surlignage.

## 0.0.9 — 2026-11-17

- Annotations en mode Éditer : surligner, souligner ou barrer du texte en le sélectionnant à la souris.
- Dessin à main levée, et formes : rectangle, ellipse, trait et flèche (Maj pour un carré, un cercle ou un angle de 45°).
- Couleur, épaisseur et remplissage se choisissent dans la barre d'outils avant de tracer, ou dans le panneau Propriétés ensuite ; les annotations se déplacent, se redimensionnent par leurs poignées, se dupliquent et s'annulent.
- La sélection de texte suit mieux le texte du document : elle ne déborde plus sur les segments voisins.

## 0.0.8 — 2026-11-17

- Sauvegarde automatique : vos ajouts (textes, images) sont mis en lieu sûr toutes les minutes, sans toucher au fichier ; l'heure de la dernière copie de secours s'affiche en bas de la fenêtre.
- Si PdfEditor se ferme brutalement, il propose au lancement suivant de reprendre vos modifications.
- Nouvelle rubrique Fichiers dans les réglages : intervalle de la sauvegarde automatique, ou la désactiver.
- Un document protégé par mot de passe n'est jamais copié en clair sur le disque par la sauvegarde automatique.

## 0.0.7 — 2026-11-16

- Ajout d'images (PNG, JPEG, WebP) en mode Éditer : bouton Image, coller avec Ctrl+V, ou glisser-déposer le fichier sur la page.
- Les images se déplacent et s'agrandissent par leurs coins, en gardant leurs proportions si vous le souhaitez ; elles se tournent d'un quart de tour et peuvent être rendues transparentes.
- La transparence des PNG est conservée, les photos JPEG sont incluses sans perte de qualité, et les photos prises en portrait s'affichent dans le bon sens.

## 0.0.6 — 2026-11-16

- Nouveau mode Éditer : cliquez sur la page pour ajouter du texte, puis choisissez police, taille, gras, italique, couleur et alignement.
- Les zones de texte se déplacent et s'élargissent à la souris, ou au millimètre près dans le panneau Propriétés ; elles se dupliquent et se suppriment.
- Annuler et rétablir chaque modification (Ctrl+Z, Ctrl+Y).
- Enregistrer (Ctrl+S) et Enregistrer sous (Ctrl+Maj+S) : le fichier n'est remplacé qu'une fois le nouveau contenu vérifié, et un document protégé garde son mot de passe.
- Avant de fermer un document modifié ou de quitter, PdfEditor propose d'enregistrer.

## 0.0.5 — 2026-11-16

- Mises à jour automatiques : PdfEditor recherche les nouvelles versions au démarrage puis toutes les 6 heures, les annonce avec leurs nouveautés, et ne les installe que sur votre clic, après vérification de leur signature.
- Après une mise à jour, un bandeau présente les nouveautés ; les notes de toutes les versions restent consultables dans les réglages.
- [Windows] Installeur pour Windows, moteur PDF inclus.

## 0.0.4 — 2026-11-16

- Page Réglages (menu ou Ctrl+,) : thème clair, sombre ou celui du système, zoom à l'ouverture, mémoire pour les pages, raccourcis clavier modifiables, détail du journal. Les réglages s'appliquent tout de suite.
- Documents récents sur l'accueil, avec leur dossier et la date d'ouverture ; option pour rouvrir le dernier document au démarrage.
- Journal de diagnostic, sans jamais le nom ni le contenu des documents ; bouton pour ouvrir son dossier.

## 0.0.3 — 2026-11-16

- Le texte des pages se sélectionne et se copie, ligne par ligne, y compris sur les pages tournées.
- Recherche dans tout le document (Ctrl+F) : sans tenir compte des majuscules ni des accents, résultats surlignés, suivant avec Entrée, précédent avec Maj+Entrée.

## 0.0.2 — 2026-11-16

- Première fenêtre : ouvrir un PDF (bouton, Ctrl+O, glisser-déposer ou « Ouvrir avec »), le lire en défilement continu.
- Miniatures des pages, plan du document, aller à une page par son numéro.
- Zoom : boutons, Ctrl + molette, Ctrl +/−, 100 % (Ctrl+0), page entière (Ctrl+1), largeur (Ctrl+2).
- Documents protégés par mot de passe : le mot de passe est demandé, une erreur de saisie est signalée.
- Thème clair ou sombre selon le système.

## 0.0.1 — 2026-11-16

- Fondations du projet : architecture, roadmap, intégration continue.
