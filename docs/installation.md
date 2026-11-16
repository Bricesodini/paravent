# Installer paravent

Testé sur macOS. Linux devrait fonctionner de la même façon, sans avoir été vérifié. Windows n'est
pas pris en charge par ce guide : la protection de la clé repose sur les droits de fichier Unix.

## Ce qu'il faut

| Besoin | Pourquoi | Vérifier |
| --- | --- | --- |
| Rust et `cargo` | compiler le binaire | `cargo --version` |
| `git` | récupérer le dépôt | `git --version` |
| Python 3 (facultatif) | brancher un outil écrit en Python | `python3 --version` |

Rust s'installe depuis <https://rustup.rs>. Aucune autre dépendance : l'adaptateur Python n'utilise
que la bibliothèque standard.

## 1. Installer le binaire

Sans copie du dépôt :

```sh
cargo install --git https://github.com/Bricesodini/paravent
```

Ou depuis une copie, utile pour lire les exemples et lancer les tests :

```sh
git clone https://github.com/Bricesodini/paravent
cd paravent
cargo install --path .
```

Le binaire arrive dans `~/.cargo/bin/`. Si `paravent --version` ne répond pas, ajoutez ce dossier à
votre `PATH`.

## 2. Créer la clé de l'installation

```sh
(umask 077; mkdir -p ~/.config/paravent; head -c 32 /dev/urandom > ~/.config/paravent/cle)
```

La clé sert à calculer les références. Elle se protège comme les données elles-mêmes :

- qui détient la clé et la liste des identifiants retrouve à quel objet répond une référence ;
- elle ne va ni dans un dépôt, ni dans une sauvegarde partagée, ni dans une conversation ;
- `paravent` refuse une clé lisible par d'autres que vous (`CLE_REFUSEE`) ;
- changer de clé change toutes les références : c'est un moyen d'oubli, pas un incident.

Une autre clé se désigne par `--cle <fichier>` ou par la variable `PARAVENT_CLE`. Une clé par
machine suffit ; deux machines qui doivent parler des mêmes références partagent la même clé.

## 3. Vérifier

Depuis une copie du dépôt :

```sh
paravent --version
paravent filtrer --declaration exemples/declaration.json < exemples/contacts.jsonl
```

La seconde commande rend des références (`contact-…`), des champs publics et des comptes. Si un
nom ou une adresse apparaît, n'allez pas plus loin et ouvrez un ticket.

Pour vérifier le dépôt entier :

```sh
cargo test
python3 -m unittest discover -s python
```

## 4. Outil en Python : copier l'adaptateur

L'adaptateur est un seul fichier. Copiez [`python/paravent.py`](../python/paravent.py) à côté de
votre outil : il trouve le binaire dans le `PATH`, ou à l'endroit donné par `PARAVENT_BIN`.

## 5. Donner la skill à vos agents

La skill [`skills/paravent/SKILL.md`](../skills/paravent/SKILL.md) dit à un agent comment
travailler aveugle. Copiez le dossier `skills/paravent` là où votre harnais cherche ses skills, par
exemple :

```sh
mkdir -p ~/.claude/skills && cp -R skills/paravent ~/.claude/skills/
```

Puis inscrivez la règle du [guide de branchement](guide-branchement.md#règle-à-inscrire-dans-les-consignes-des-agents)
dans les consignes que lisent vos agents (`AGENTS.md`, `CLAUDE.md`). Sans cette règle, rien ne dit
à l'agent de passer par l'outil plutôt que d'ouvrir la base.

## Et ensuite

Brancher un premier outil : [`guide-branchement.md`](guide-branchement.md).

## Mettre à jour

```sh
cargo install --git https://github.com/Bricesodini/paravent --force
```

La clé n'est pas touchée : vos références restent les mêmes. Après une mise à jour, rejouez le banc
de chaque outil branché.

## Désinstaller

```sh
cargo uninstall paravent
```

La clé reste dans `~/.config/paravent/`. La supprimer rend toutes les références existantes
inutilisables ; c'est irréversible.

## En cas de problème

| Message | Cause | Remède |
| --- | --- | --- |
| `command not found: paravent` | `~/.cargo/bin` absent du `PATH` | l'ajouter, ou appeler `~/.cargo/bin/paravent` |
| `PARAVENT_ERREUR code=CLE_REFUSEE` | clé absente, trop courte ou lisible par d'autres | recréer la clé avec la commande de l'étape 2, ou `chmod 600` |
| `BINAIRE_ABSENT` (adaptateur Python) | le binaire n'est pas trouvé | l'installer, ou renseigner `PARAVENT_BIN` |
| `BINAIRE_TROP_ANCIEN` | binaire plus vieux que l'adaptateur | mettre à jour le binaire |

Tous les codes : [`codes.md`](codes.md).
