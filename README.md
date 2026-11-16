# paravent

**Un agent agit sur des données personnelles sans les voir.**

> **In English.** `paravent` is a deterministic filter between an AI agent and personal data. Each
> tool declares which fields are public, personal or secret; anything undeclared is treated as
> personal. The agent only receives opaque references, public fields, presence flags, counts and
> states. To act on a person, it proposes a pending action with a fill-in template that a human
> validates where the data is visible. A leak bench proves that no planted value escapes. The
> tool, its commands and its documentation are in French.

## Le problème

Un modèle de langage qui pilote un outil en ligne de commande lit tout ce que cet outil affiche.
Dès que l'outil touche une base où figurent des personnes (noms, courriels, téléphones), ces
données partent chez le fournisseur du modèle.

`paravent` l'empêche sans retirer à l'agent sa capacité d'agir. Il reçoit des **références
opaques**, des **comptes** et des **états**, jamais les valeurs.

```
vous : « envoie un rappel à Sophie, de l'école des Tilleuls »
agent → outil : chercher "Sophie" "Tilleuls"
outil → agent : contact-f8z00rja   rôle="enseignante"   courriel:renseigné
agent → outil : proposer un message pour contact-f8z00rja : « Bonjour {prénom}, … »
vous, dans votre application : destinataire et texte en clair, vous validez, le message part
```

## Comment ça marche

| Brique | Rôle |
| --- | --- |
| **Déclaration** | Un fichier JSON où chaque outil classe ses champs : publics, personnels, secrets. Ce qui n'est pas déclaré est traité comme personnel. |
| **Filtre** | `paravent filtrer` remplace chaque identifiant par une référence, laisse passer les champs publics et réduit les champs personnels à leur présence (`courriel:renseigné`). |
| **Modèle à trous** | L'agent écrit `Bonjour {prénom}` ; le texte est rempli du côté de la source, hors de sa vue. |
| **Action en attente** | Ce que l'agent propose ne part pas : un humain valide là où il voit les données en clair. |
| **Banc anti-fuite** | `paravent banc` joue toutes les commandes d'un outil sur des données fictives semées de valeurs pièges, et échoue à la première qui sort. |

Tout est déterministe : aucune détection statistique, aucun modèle. `paravent` s'applique à des
données structurées dont on connaît les champs.

## Installer

Il faut Rust (`cargo`). Détail, clé, mise à jour et installation de la skill :
[`docs/installation.md`](docs/installation.md).

```sh
cargo install --git https://github.com/Bricesodini/paravent
(umask 077; mkdir -p ~/.config/paravent; head -c 32 /dev/urandom > ~/.config/paravent/cle)
paravent --version
```

## Essayer en deux minutes

Depuis une copie du dépôt :

```sh
paravent filtrer --declaration exemples/declaration.json < exemples/contacts.jsonl
```

```
contact-f8z00rja	rôle="enseignante"	état="actif"	courriel:renseigné	nom:renseigné	prénom:renseigné
…
# comptes
contact	3
contact.état=actif	2
contact.état=parti	1
```

Les références dépendent de votre clé : les vôtres seront différentes. Aucun nom, aucune adresse.

Deux exemples complets :

- un outil Python branché, avec son plan de banc : [`exemples/outil_python/`](exemples/outil_python/) ;
- une application qui ne répond qu'en forme sûre, de la recherche à la validation humaine :
  `cargo run --example source_aveugle`.

## Brancher vos outils

| Vous avez… | Chemin |
| --- | --- |
| un outil existant, ou une source que vous ne développez pas | Niveau 1 : l'outil passe ses sorties au filtre |
| une application que vous développez | Niveau 2 : l'application ne répond qu'en forme sûre |

Le guide pas à pas, avec la liste de contrôle de relecture d'une déclaration et la règle à inscrire
dans les consignes de vos agents : [`docs/guide-branchement.md`](docs/guide-branchement.md).

Pour les agents eux-mêmes : la skill [`skills/paravent/SKILL.md`](skills/paravent/SKILL.md).

## Documentation

| Document | Contenu |
| --- | --- |
| [`docs/installation.md`](docs/installation.md) | Installer, créer la clé, vérifier, mettre à jour, désinstaller. |
| [`docs/guide-branchement.md`](docs/guide-branchement.md) | Brancher un outil ou une application, faire relire la déclaration, passer le banc. |
| [`docs/codes.md`](docs/codes.md) | Commandes, formats d'entrée et de sortie, codes d'erreur. |
| [`docs/cadrage.md`](docs/cadrage.md) | Vocabulaire, garanties, limites. |
| [`docs/adr/`](docs/adr/) | Format des références ; modèles à trous et actions en attente. |

## Ce que ce n'est pas

- **Pas un coffre.** Un agent qui a un terminal peut lire un fichier lisible sur la machine.
  `paravent` garantit ce qui sort des outils branchés, pas les droits d'accès du poste : retirez aux
  agents l'accès direct à la base, aux exports et à la clé.
- **Pas un détecteur.** Il ne cherche pas de données personnelles dans du texte libre. Un texte
  écrit par une personne est personnel en entier : l'agent n'en connaît que la présence.
- **La sûreté dépend de la déclaration.** Un champ classé public à tort sort tel quel. La relecture
  de la déclaration par un humain fait partie de la mise en service.
- **Petits nombres.** « Un seul contact sans courriel dans cette école » désigne quelqu'un. Cette
  version ne traite pas ce cas ; elle le documente.
- **Ce que vous tapez passe par le modèle.** Un prénom dicté à l'agent n'est pas masqué.

## État du projet

Version 0.1.0 : bibliothèque Rust, binaire (`filtrer`, `modele`, `banc`), adaptateur Python,
contrat des actions en attente. Testé sur macOS ; pas encore vérifié sous Linux ni Windows.
L'interface peut encore changer avant une version 1.

Vérifier une copie du dépôt :

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
python3 -m unittest discover -s python
```

## Licence

[MIT](LICENSE).
