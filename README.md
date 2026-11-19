# PdfEditor

Éditeur de PDF libre (MIT) pour **Windows**, **Linux** (à construire soi-même)
et **Android**, écrit en Rust. Tout se fait sur votre machine : vos documents
ne la quittent que si vous lancez vous-même une action IA, vers le fournisseur
que vous avez choisi.

- **Lire** : rendu fidèle (moteur de Chrome), recherche, signets, miniatures.
- **Éditer** : texte, images, annotations, formes, notes ; modifier le texte
  existant ; **caviarder** pour de vrai (le contenu est retiré du fichier).
- **Formulaires** : remplir un formulaire PDF, ou **en créer un** — texte,
  date, montant, e-mail, téléphone, cases, listes, signatures, totaux
  calculés — sur une page vierge, un PDF ou un document **Word importé** ;
  détection automatique des champs d'un formulaire « papier ».
- **Pages** : insérer, supprimer, pivoter, réordonner, fusionner, extraire.
- **Signer** : signature numérique **PAdES**, signature manuscrite, horodatage,
  **verrouillage** du document ; vérification des signatures reçues.
- **IA** (facultative, fournisseur au choix) : résumé, questions avec
  renvoi aux pages, **traduction** mise en page conservée, **modification**
  par consigne, anonymisation, assistant d'actions — toujours avec aperçu et
  accord avant d'appliquer quoi que ce soit.

> Projet en développement actif : la vue d'ensemble est dans
> [docs/ROADMAP.md](docs/ROADMAP.md), le détail (bogues, limites, points à
> vérifier) dans les [issues](https://github.com/Aquadrox/PdfEditor/issues).
> Un problème ? [Signalez-le](https://github.com/Aquadrox/PdfEditor/issues/new/choose)
> (sans document confidentiel ni clé API : le dépôt est public).

## Sommaire

1. [Installer](#installer)
2. [Prise en main](#prise-en-main)
3. [Formulaires](#formulaires)
4. [Importer un document Word](#importer-un-document-word)
5. [Configurer l'IA (clé API)](#configurer-lia-clé-api)
6. [Fichiers, réglages et confidentialité](#fichiers-réglages-et-confidentialité)
7. [Construire depuis les sources](#construire-depuis-les-sources)
8. [Documentation](#documentation) · [Licence](#licence)

## Installer

### Windows

Télécharger `PdfEditor_X.Y.Z_x64-setup.exe` depuis les
[Releases](https://github.com/Aquadrox/PdfEditor/releases) et le lancer.
Ensuite, PdfEditor propose lui-même ses mises à jour : un bandeau les
annonce, elles sont vérifiées par signature et installées sur votre clic.

**En faire le lecteur PDF par défaut** : l'installeur déclare PdfEditor pour
les fichiers `.pdf` (il apparaît dans « Ouvrir avec »). Windows réserve le
choix du lecteur par défaut à l'utilisateur : **Réglages → Général →
Choisir PdfEditor par défaut…** ouvre la page « Applications par défaut » de
Windows ; cherchez `.pdf` et choisissez PdfEditor. (Ou : clic droit sur un
PDF → Ouvrir avec → Choisir une autre application → PdfEditor → Toujours.)

### Android (téléphones ARM 64 bits)

Installer `PdfEditor_X.Y.Z_android.apk` depuis les Releases (autoriser
l'installation d'applications depuis le navigateur). Les versions suivantes
s'annoncent dans PdfEditor : **Mettre à jour** télécharge, vérifie et installe.

- La première fois, Android demande d'autoriser PdfEditor à installer des
  applications ; l'installation reprend au retour.
- Selon le téléphone (Xiaomi…), une fenêtre de confirmation d'Android
  s'ouvre : toucher **Mettre à jour**.
- Android ferme PdfEditor le temps de l'installation ; une notification le
  rouvre. Documents et réglages sont gardés.
- L'APK de test des PR (« PdfEditor (test) ») s'installe à côté et ne se met
  pas à jour.

La version Android est récente : son interface tactile est encore en
chantier (voir la roadmap).

### Linux

Pas encore de paquet : voir [Construire depuis les sources](#construire-depuis-les-sources).

## Prise en main

Ouvrez un PDF (bouton **Ouvrir**, glisser-déposer, ou « Ouvrir avec
PdfEditor »), ou partez de **Nouveau document** / **Importer un document
Word** sur l'écran d'accueil. Les **modes**, en haut de la fenêtre,
n'affichent chacun que leurs outils :

| Mode | Pour… |
|---|---|
| **Lire** | lire, chercher (Ctrl+F), remplir les formulaires |
| **Éditer** | ajouter texte, images, annotations, formes ; modifier le texte du document ; caviarder |
| **Formulaire** | créer ou modifier les champs d'un formulaire |
| **Pages** | organiser les pages, insérer d'autres PDF, extraire |
| **Signer** | signer, verrouiller, vérifier les signatures |
| **IA** | analyser, questionner, traduire, modifier le document |

Tout s'annule (**Ctrl+Z** / **Ctrl+Y**) ; le fichier d'origine n'est jamais
modifié avant **Enregistrer** (Ctrl+S), et une copie de secours protège
votre travail en cas de plantage.

- **Ctrl+K** : palette de commandes, toutes les actions par leur nom.
- **F1** : « Comment faire ? » — décrivez ce que vous voulez faire, PdfEditor
  montre l'outil dans la barre.
- **Ctrl+,** : Réglages (thème, zoom, raccourcis, signature, IA…). Tous les
  raccourcis sont modifiables.

## Formulaires

**Remplir** : en mode **Lire**, cliquez dans les champs. Les champs
« intelligents » vérifient la saisie : une date se tape librement
(« 19/11/26 », « 2026-11-19 ») ou se choisit au calendrier, un montant
s'affiche « 1 234,50 € », un total se calcule tout seul. Les champs
obligatoires sont signalés. **Menu ≡ → Aplatir le formulaire** fige les
valeurs dans la page.

**Créer ou modifier** : mode **Formulaire**.

1. Choisissez un outil (Texte, Zone, Date, Montant, E-mail, Téléphone, Case,
   Choix, Liste déroulante, Liste, Signature), puis tracez un cadre sur la
   page — ou cliquez pour la taille habituelle.
2. Réglez le champ dans le panneau de droite : nom, libellé, format,
   obligatoire, options d'une liste, calcul (somme, moyenne…) à partir
   d'autres champs.
3. Déplacez ou redimensionnez à la souris (flèches pour décaler, Suppr pour
   supprimer).
4. **Détecter les champs** propose les champs d'un formulaire « papier »
   (traits `____`, pointillés, cases ☐, « Nom : ») ; cochez ceux à garder.
5. **Tester** passe en mode Lire pour remplir le formulaire ; **Enregistrer**.

Les champs suivent les conventions d'Acrobat : dates, montants et totaux
fonctionnent aussi dans Adobe Reader, Chrome ou Edge. Un **emplacement de
signature** se signe ensuite dans le mode **Signer**.

## Importer un document Word

**Importer un document Word** (accueil ou menu ≡) accepte `.docx`, `.doc`,
`.odt` et `.rtf` ; « Ouvrir » et le glisser-déposer aussi. Le document est
converti en PDF et s'ouvre **sans titre** : **Enregistrer** demande où
l'écrire.

La conversion utilise, dans l'ordre :

1. **Microsoft Word** s'il est installé (Windows) — le plus fidèle ;
2. sinon **LibreOffice** (gratuit, [libreoffice.org](https://www.libreoffice.org)) ;
3. sinon, pour un `.docx` seulement, l'**import intégré** de PdfEditor :
   texte, titres, gras, listes et tableaux dans une mise en page simplifiée,
   sans les images (PdfEditor le signale).

Pour `.doc`, `.odt` et `.rtf`, Word ou LibreOffice est indispensable.
L'import n'est pas encore disponible sur Android.

## Configurer l'IA (clé API)

L'IA est **facultative** : sans elle, tout le reste fonctionne. PdfEditor
parle à n'importe quel service **compatible avec l'API d'OpenAI**
(`/chat/completions`) : DeepSeek, OpenAI, Mistral, GLM (Z.ai), OpenRouter,
ou un modèle **local** (Ollama, LM Studio) qui ne fait rien sortir de votre
PC. Le service se facture chez le fournisseur, selon votre usage.

### Pas à pas

1. **Créez une clé API** chez le fournisseur (tableau ci-dessous) et
   gardez-la sous la main. Inutile pour Ollama et LM Studio.
2. Ouvrez **Menu ≡ → Réglages…** (Ctrl+,), rubrique **IA**.
3. **Fournisseur** : choisissez le préréglage. Il remplit l'adresse, le
   modèle proposé, la taille des requêtes et, quand il est connu, le tarif.
   Tout reste modifiable.
4. **Adresse de l'API** : laissez celle du préréglage. Ne collez *pas*
   `/chat/completions` à la fin (PdfEditor l'ajoute et le retire s'il a été
   recopié).
5. **Modèle** : son nom exact chez le fournisseur. En cas de doute, faites
   l'étape 7 d'abord : **Tester** affiche les modèles du serveur, un clic en
   choisit un.
6. **Clé API** : collez-la, puis cliquez **Enregistrer la clé** — ce bouton,
   à côté du champ, est **distinct** du bouton Enregistrer des Réglages. Le
   message « Clé enregistrée dans le trousseau du système. » confirme. La clé
   ne s'affiche plus ensuite (le champ reste vide : c'est normal).
7. Cliquez **Tester** (« Tester ces réglages ») : une très courte requête
   (« Réponds OK ») et la liste des modèles. « Réglages valides. » : c'est
   prêt. Sinon, des pistes de correction s'affichent (voir *Dépannage*).
8. **Tarif par million de jetons** (entrée, entrée en cache, sortie, devise
   en 3 lettres : `USD`, `EUR`) : recopiez-le depuis la page de prix du
   fournisseur. Il ne sert qu'à **estimer le coût avant l'envoi** ; à 0,
   l'estimation affiche 0.
9. Cliquez **Enregistrer** en bas des Réglages.

Ensuite : mode **IA**, choisissez la portée (sélection, page, pages,
document) et l'action.

### Fournisseurs préréglés

| Préréglage | Adresse de l'API | Clé à créer sur | À savoir |
|---|---|---|---|
| DeepSeek | `https://api.deepseek.com` | [platform.deepseek.com](https://platform.deepseek.com) | Tarif pré-rempli (heures pleines). Paramètre supplémentaire pré-rempli : `{"thinking":{"type":"disabled"}}` (réponses directes, moins chères) |
| OpenAI | `https://api.openai.com/v1` | [platform.openai.com](https://platform.openai.com/api-keys) | Modèle et tarif à renseigner : nom exact, ou choisi par **Tester** |
| Mistral | `https://api.mistral.ai/v1` | [console.mistral.ai](https://console.mistral.ai) | Modèle proposé : `mistral-small-latest` ; tarif à renseigner |
| GLM (Z.ai) | `https://api.z.ai/api/paas/v4` | [z.ai](https://z.ai) | Même paramètre supplémentaire que DeepSeek ; tarif à renseigner |
| OpenRouter | `https://openrouter.ai/api/v1` | [openrouter.ai](https://openrouter.ai/keys) | Modèle au format `fournisseur/modèle` (ex. `mistralai/…`) ; donne accès à beaucoup de modèles avec une seule clé |
| Ollama | `http://localhost:11434/v1` | — (pas de clé) | Installez [Ollama](https://ollama.com), téléchargez un modèle (`ollama pull <modèle>`) ; **Tester** liste les modèles installés. Rien ne sort de votre PC ou réseau |
| LM Studio | `http://localhost:1234/v1` | — (pas de clé) | Chargez un modèle et démarrez le serveur local de LM Studio |
| Autre (compatible OpenAI) | adresse libre | selon le service | Tout service qui expose `/chat/completions` (et idéalement `/models`) |

Un modèle local demande une machine assez puissante ; les préréglages
locaux limitent les requêtes à 8 000 jetons (réglable).

### Règles de sécurité de la clé

- La clé est rangée dans le **trousseau du système**, jamais dans
  `config.toml` ni dans les journaux :
  - **Windows** : Gestionnaire d'identifiants (Panneau de configuration →
    Gestionnaire d'identifiants → Informations d'identification Windows),
    entrée `PdfEditor/ai-key/<hôte>` ;
  - **Linux** : trousseau de la session (GNOME Keyring, KWallet — service
    Secret Service, qui doit être actif), libellé `PdfEditor/ai-key/<hôte>`.
- **Une clé par hôte** (`api.deepseek.com`, `api.openai.com`…) : deux profils
  vers le même service partagent la clé. Changer d'adresse demande la clé du
  nouveau service.
- L'adresse doit être en **`https://`**. `http://` n'est accepté que vers ce
  PC (`localhost`, `127.0.0.1`) ou le réseau local (`192.168.…`, `10.…`,
  `*.local`).
- La clé n'est **envoyée qu'en HTTPS, ou en HTTP vers ce PC**. Vers un
  serveur `http://` du réseau local, elle n'est jamais envoyée (Ollama et
  LM Studio n'en ont pas besoin).
- **Retirer la clé** : videz le champ **Clé API** puis **Enregistrer la clé**
  (« Clé retirée du trousseau. »).
- **Android** : le trousseau du téléphone n'est pas encore branché ; seul un
  serveur local sans clé est utilisable pour l'instant.

### Ce qui est envoyé, et quand

- Rien ne part sans une action IA lancée par vous, et seulement le **texte**
  de la portée choisie (jamais le fichier PDF, ni les images).
- Le **premier envoi** vers un fournisseur montre ce qui part et où, et
  demande votre accord (une fois par fournisseur ; jamais pour un serveur
  local).
- Avant chaque action, le **coût estimé** s'affiche. Au-delà de **Confirmer
  au-delà de** (0,05 par défaut, dans la devise du tarif), PdfEditor demande
  confirmation ; **Confirmer chaque envoi** le demande toujours.
- **Plafond mensuel** (0 = aucun) : une fois atteint, plus aucun envoi ce
  mois-ci. La **Consommation** (jour, mois, total) est comptée dans
  `ai-usage.json` — des compteurs seulement, aucun texte.
- Le texte du document est toujours traité comme une **donnée** : une
  consigne cachée dans un PDF n'est pas exécutée, et aucune modification
  n'est appliquée sans aperçu ni accord.

### Réglages avancés

- **Profils** : enregistrez plusieurs fournisseurs (« Enregistrer ce
  fournisseur comme profil ») et passez de l'un à l'autre avec **Utiliser**.
- **Profil de secours** : essayé automatiquement si le fournisseur principal
  ne répond pas.
- **Délai maximal d'une requête** (5 à 900 s, 120 par défaut) : à augmenter
  pour un modèle local lent.
- **Jetons par requête** (2 000 à 1 000 000) : au-delà, le document est lu
  par parties puis synthétisé. Ne dépassez pas la fenêtre de contexte du
  modèle.
- **Paramètres supplémentaires** : un objet JSON ajouté tel quel à chaque
  requête, propre au fournisseur (ex. `{"temperature": 0.2}`). Laissez vide
  si vous ne savez pas.
- **Consignes** : les instructions de chaque action (résumé, points clés…)
  sont modifiables ; vide = consigne intégrée. Les règles de sécurité restent
  toujours en tête.
- **Recettes** (rubrique Recettes) : consignes réutilisables pour l'assistant
  d'actions, avec les seules actions qu'elles peuvent proposer ; exportables.
- **Traduction** : langue proposée, corps minimal, glossaire
  (`terme = traduction`, ou `terme` seul pour ne pas le traduire).

### Dépannage

| Message ou symptôme | Cause probable | Que faire |
|---|---|---|
| HTTP 401 / 403 : clé API refusée | Clé absente, mal collée, révoquée, ou d'un autre service | Recollez la clé puis **Enregistrer la clé** ; vérifiez qu'elle correspond bien à l'adresse choisie |
| HTTP 404 / 405 : introuvable | Adresse ou nom de modèle erroné | Reprenez l'adresse du préréglage ; choisissez le modèle avec **Tester** |
| HTTP 402 / 429 | Crédit épuisé ou limite de débit | Rechargez le compte chez le fournisseur ; réessayez plus tard |
| Délai dépassé | Document long, modèle lent ou serveur local surchargé | Réduisez la portée, augmentez le délai, baissez « Jetons par requête » |
| « adresse en https:// obligatoire » | Adresse `http://` vers Internet | Utilisez l'adresse `https://` du service |
| « Aucune clé pour ce fournisseur » alors qu'elle a été collée | **Enregistrer la clé** n'a pas été cliqué | Recollez-la et cliquez **Enregistrer la clé** |
| Linux : erreur du trousseau à l'enregistrement de la clé | Aucun service Secret Service actif | Lancez GNOME Keyring ou KWallet (ou un autre trousseau Secret Service), puis réessayez |
| L'assistant (« Action ») explique au lieu de proposer | Le modèle ne gère pas l'appel d'outils | Choisissez un modèle récent qui le gère ; l'analyse, les questions et la traduction marchent quand même |
| Estimation de coût à 0 | Tarif non renseigné | Remplissez le tarif par million de jetons |

## Fichiers, réglages et confidentialité

| Contenu | Windows | Linux |
|---|---|---|
| Réglages (`config.toml`) | `%APPDATA%\PdfEditor` | `~/.config/pdfeditor` |
| Données : récents, copies de secours, identités de signature, signatures manuscrites, recettes, brouillons, consommation IA | `%LOCALAPPDATA%\PdfEditor` | `~/.local/share/pdfeditor` |
| Journaux | `…\PdfEditor\logs` (dossier des données) | `~/.local/share/pdfeditor/logs` |

- `config.toml` se lit et se modifie à la main ; une valeur invalide est
  signalée et remplacée par sa valeur par défaut. La clé API n'y est
  **jamais** (elle est dans le trousseau, voir plus haut).
- La variable d'environnement `PDE_HOME=<dossier>` place réglages, données
  et journaux dans un seul dossier (version portable, essais).
- Les journaux ne contiennent jamais de nom ni de contenu de document, ni de
  texte envoyé à l'IA.
- Les clés privées de signature sont chiffrées par le système (DPAPI sous
  Windows, trousseau sous Linux) ; un certificat `.p12` / `.pfx` n'est
  demandé qu'à l'import.

## Construire depuis les sources

Rust stable (voir `rust-toolchain.toml`). Le moteur PDF a besoin de la
bibliothèque PDFium, téléchargée (version figée, empreinte vérifiée) dans
`vendor/pdfium/` par :

```bash
./scripts/fetch-pdfium.sh
```

```bash
cargo test --workspace
```

Lancer l'application (sous Linux, paquets `libwebkit2gtk-4.1-dev` et
`librsvg2-dev` nécessaires) :

```bash
cargo run -p pdfeditor -- chemin/vers/document.pdf
```

Les tests de signature contre-vérifient les PDF signés avec `pdfsig` et
`qpdf` s'ils sont installés (paquets `poppler-utils` et `qpdf`) ; si
LibreOffice est installé, un test essaie aussi la conversion réelle d'un
document.

Tests de l'interface (Node 22) :

```bash
cd apps/desktop/ui && node --test
```

```bash
cd apps/desktop/ui-tests && npm ci && npm test
```

Règles de contribution (PR, titres, version, secrets) :
[CONTRIBUTING.md](CONTRIBUTING.md).

## Documentation

| Document | Contenu |
|---|---|
| [docs/FONCTIONS.md](docs/FONCTIONS.md) | Liste des fonctions et priorités |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Couches, crates, choix techniques, tests, confidentialité |
| [docs/DESIGN.md](docs/DESIGN.md) | Direction visuelle, jetons, composants, rédaction |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Étapes et avancement (vue d'ensemble) |
| [Issues](https://github.com/Aquadrox/PdfEditor/issues) | Bogues, limites restantes, points à vérifier |
| [CHANGELOG.md](CHANGELOG.md) | Notes de version |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Règles de contribution, protection de `main`, secrets |

## Licence

MIT. PDFium (moteur PDF) est distribué sous licence BSD-3 / Apache-2.0 ; les
polices embarquées (Noto, IBM Plex Sans, Caveat, Dancing Script) sous licence
OFL, Homemade Apple sous licence Apache-2.0.
