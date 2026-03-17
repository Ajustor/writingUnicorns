# 🦄 Writing Unicorns

Un IDE léger et performant construit en Rust, inspiré de VSCode.  
Consommation RAM cible : **30–80 MB** contre 300–500 MB pour VSCode.

---

## Fonctionnalités

- ✏️ Éditeur de texte avec coloration syntaxique (Rust, TypeScript, JavaScript, Python, JSON, TOML, Shell, Dockerfile)
- 🗂️ Arbre de fichiers avec icônes et navigation par dossiers
- 🖥️ Terminal intégré (PTY réel, multi-onglets, 256 couleurs, historique shell)
- 🔍 Command palette (`Ctrl+P`) avec recherche floue
- 🌿 Intégration Git (statut des fichiers, branche courante)
- 🧩 Système d'extensions (manifest TOML, installation depuis git, template)
- ⚙️ Paramètres configurables : thème, taille de police, raccourcis clavier
- 🎨 Thèmes personnalisables avec sélecteurs de couleurs RGB
- 🔢 Multi-curseurs (`Ctrl+D`, `Ctrl+Shift+L`, `Alt+↑/↓`, `Ctrl+Clic`)
- 💾 Indicateur de fichier modifié + auto-save optionnel

---

## Prérequis

### Linux / macOS
- [Rust](https://rustup.rs/) 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Bibliothèques système (Linux) :
  ```bash
  # Ubuntu / Debian
  sudo apt install libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev \
                   libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
  # Fedora
  sudo dnf install gtk3-devel libxcb-devel xkeyboard-config-devel openssl-devel
  ```

### Windows
- [Rust](https://rustup.rs/) 1.75+ (installer `.msi`)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/fr/visual-cpp-build-tools/) avec le composant C++

---

## Installation

### Depuis les sources

```bash
git clone https://github.com/votre-utilisateur/writingUnicorns
cd writingUnicorns
cargo build --release
```

Le binaire se trouve dans `target/release/writing-unicorns` (ou `.exe` sur Windows).

### Lancer directement

```bash
cargo run --release
```

### Depuis une release GitHub

Téléchargez le binaire pour votre plateforme depuis la page [Releases](../../releases) et rendez-le exécutable :

```bash
chmod +x writing-unicorns-linux-x86_64
./writing-unicorns-linux-x86_64
```

---

## Utilisation

### Premier lancement

1. Lancez l'application
2. **File → Open Folder…** (ou `Ctrl+O`) pour ouvrir un projet
3. Cliquez sur un fichier dans la barre latérale pour l'ouvrir

### Interface

```
[Barre d'activité] [Sidebar] [Éditeur]
                              [Terminal]
```

| Zone | Description |
|------|-------------|
| **Barre d'activité** (gauche) | Icônes Explorer / Recherche / Git / Extensions |
| **Sidebar** | Arbre de fichiers, statut Git, panel Extensions |
| **Éditeur** | Zone principale avec onglets et coloration |
| **Terminal** | Terminal intégré (bas), multi-onglets |

### Raccourcis clavier par défaut

| Raccourci | Action |
|-----------|--------|
| `Ctrl+O` | Ouvrir un dossier |
| `Ctrl+Shift+O` | Ouvrir un fichier |
| `Ctrl+N` | Nouveau fichier |
| `Ctrl+S` | Sauvegarder |
| `Ctrl+W` | Fermer l'onglet |
| `Ctrl+P` | Command palette / recherche de fichier |
| `Ctrl+B` | Afficher/masquer la sidebar |
| `Ctrl+\`` | Afficher/masquer le terminal |
| `Ctrl+F` | Rechercher dans le fichier |
| `Ctrl+Z` | Annuler |
| `Ctrl+Y` | Rétablir |
| `Ctrl+,` | Ouvrir les paramètres |
| `F1` | Aide sur les raccourcis |

> Les raccourcis sont entièrement configurables dans **Settings → Keybindings**.

### Terminal intégré

- **`+`** : ouvrir un nouvel onglet terminal
- **`Ctrl+C`** : interrompre le processus en cours
- **`Ctrl+D`** : EOF / quitter le shell
- **`Tab`** : complétion automatique du shell
- **`↑` / `↓`** : naviguer dans l'historique des commandes

### Multi-curseurs

| Raccourci | Action |
|-----------|--------|
| `Ctrl+Clic` | Ajouter un curseur |
| `Ctrl+D` | Sélectionner la prochaine occurrence |
| `Ctrl+Shift+L` | Sélectionner toutes les occurrences |
| `Alt+↑` / `Alt+↓` | Ajouter un curseur au-dessus/en-dessous |
| `Échap` | Revenir à un seul curseur |

---

## Configuration

Le fichier de configuration est stocké dans :

| OS | Chemin |
|----|--------|
| Linux | `~/.config/writing-unicorns/config.toml` |
| macOS | `~/Library/Application Support/writing-unicorns/config.toml` |
| Windows | `%APPDATA%\writing-unicorns\config.toml` |

Exemple de `config.toml` :

```toml
[theme]
name = "dark"
background = [30, 30, 30]
foreground = [212, 212, 212]
accent = [0, 122, 204]

[editor]
tab_size = 4
insert_spaces = true
word_wrap = false
line_numbers = true
auto_save = false

[font]
size = 14.0
family = "monospace"

[keybindings]
# Chaque raccourci : key + modificateurs
save = { key = "S", ctrl = true, shift = false, alt = false }
open_folder = { key = "O", ctrl = true, shift = false, alt = false }
# ... (généré automatiquement par l'éditeur de raccourcis)
```

---

## Système d'extensions

Les extensions sont stockées dans `~/.config/writing-unicorns/extensions/`.

### Installer une extension depuis git

Dans le panel **Extensions** (icône puzzle dans la barre d'activité) :
1. Collez l'URL du dépôt git
2. Cliquez **Install**
3. L'extension est clonée, compilée et chargée

### Créer une extension

Via le panel Extensions → **Create Extension Template** :
1. Entrez un nom
2. Cliquez **Generate** → un projet Cargo est créé dans `~/extensions/<nom>/`
3. Implémentez le trait Plugin dans `src/lib.rs`
4. Compilez : `cargo build --release`
5. Copiez `target/release/lib<nom>.so` + `manifest.toml` dans `~/.config/writing-unicorns/extensions/<id>/`

### Modules de langages (développement)

Des modules de langages indépendants et testables sont disponibles dans `../modules/` :

```bash
cd ../modules
cargo test                    # tester tous les modules
cargo test -p rust-lang       # tester uniquement Rust
cargo test -p typescript-lang # tester uniquement TypeScript
```

---

## Compilation CI/CD

Les workflows GitHub Actions sont dans `.github/workflows/` :

- **`ci.yml`** : `cargo check` + `clippy -D warnings` + `fmt --check` sur Ubuntu, macOS, Windows
- **`release.yml`** : build des binaires sur tag `v*` pour 4 cibles :
  - `x86_64-unknown-linux-gnu`
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
  - `x86_64-pc-windows-msvc`

---

## Technologies

| Composant | Technologie |
|-----------|-------------|
| UI | [egui](https://github.com/emilk/egui) / [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) 0.31 |
| Buffer texte | [ropey](https://github.com/cessen/ropey) |
| Terminal PTY | [portable-pty](https://github.com/wez/wezterm/tree/main/pty) |
| Parsing ANSI | [vte](https://github.com/alacritty/vte) |
| Icônes | [egui-phosphor](https://github.com/lucasmerlin/hello_egui/tree/main/crates/egui-phosphor) |
| Git | [git2](https://github.com/rust-lang/git2-rs) |
| Dialogues fichiers | [rfd](https://github.com/PolyMeilex/rfd) |
| Extensions dynamiques | [libloading](https://github.com/nagisa/rust_libloading) |

---

## Licence

MIT
