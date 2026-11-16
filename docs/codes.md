# Commandes, codes de sortie et messages

Ces codes sont stables : un agent ou un script peut s'y fier. Un code ne change pas de sens ; un
cas nouveau reçoit un code nouveau.

## `paravent filtrer`

```sh
outil lister | paravent filtrer --declaration declaration.json [--cle <fichier>] [--json]
```

- Entrée : un objet JSON par ligne, `{"type": …, "id": …, "champs": {…}}`. Une source qui attribue
  ses propres références envoie `"ref"` à la place de `"id"`. Les lignes blanches sont ignorées.
- Clé : `--cle`, sinon la variable `PARAVENT_CLE`, sinon `~/.config/paravent/cle`.
- Sortie texte, une ligne par objet, champs séparés par une tabulation : la référence, puis
  `champ=valeur` pour un champ public (valeur en JSON), puis `champ:renseigné` ou `champ:vide` pour
  un champ personnel. Elle se termine par `# comptes` : `type<TAB>total`, puis
  `type.champ=valeur<TAB>nombre` pour chaque champ listé dans `comptes` par la déclaration.
- Sortie `--json` : un objet par ligne, `{"ref", "type", "publics", "personnels"}`, puis
  `{"comptes": {type: {"total": n, champ: {valeur: n}}}}`.
- Au premier refus, le filtre s'arrête. Les lignes déjà écrites restent ; les comptes ne sont pas
  écrits. Un refus ne recopie jamais la ligne reçue.

## `paravent modele`

```sh
echo 'Bonjour {prénom}, vous êtes {rôle}.' | paravent modele --declaration declaration.json --type contact
```

Contrôle un modèle à trous lu sur l'entrée : syntaxe, type déclaré, aucun champ secret ni non
déclaré. Rend `MODELE_OK type=<type> champs=<champ>,<champ>`. Une accolade littérale se double.
Cette commande ne remplit rien et n'a pas besoin de la clé : le remplissage est une fonction de la
bibliothèque, appelée du côté de la source. Contrat des actions en attente :
`docs/adr/0002-actions-en-attente.md`.

## `paravent banc`

```sh
paravent banc --plan plan.json
```

Le plan :

```json
{
  "pieges": ["PIEGE-prenom-7f3a", "piege.courriel@exemple.invalid"],
  "commandes": [["outil", "lister"], ["outil", "chercher", "PIEGE-prenom-7f3a"]],
  "surveiller": ["sorties", "outil.log"],
  "delai_secondes": 60
}
```

- `pieges` : des valeurs reconnaissables, d'au moins 8 octets, que l'auteur de l'outil a semées dans
  son jeu de données fictif, une par champ personnel.
- `commandes` : toutes les commandes de l'outil, y compris celles qui échouent. Chacune est une
  liste d'arguments, lancée sans interpréteur depuis le dossier du plan.
- `surveiller` : les fichiers et dossiers où l'outil écrit, journal compris. Seuls les fichiers
  créés ou modifiés par une commande sont fouillés.
- `delai_secondes` : durée maximale d'une commande (60 par défaut).

Chaque piège est cherché sur la sortie standard, la sortie d'erreur, dans les fichiers créés ou
modifiés et dans leur nom. Formes cherchées : `clair`, `casse` (autre casse), `url` (encodage URL),
`json` (échappement `\uXXXX`), `base64` (alphabets usuel et URL, tout alignement, lignes coupées
ou non).

Sortie, sur la sortie standard :

```
FUITE commande=<n> emplacement=<stdout|stderr|fichier:<chemin>|nom-de-fichier> piege=<n> variante=<forme>
BANC_ECHEC commandes=<n> pieges=<n> fuites=<n>
```

ou `BANC_OK commandes=<n> pieges=<n>`. `commande` et `piege` sont des rangs dans le plan, à partir
de 1. Le banc ne réaffiche ni un piège ni ce qu'une commande a écrit.

Ce que le banc ne voit pas : une valeur transformée autrement (chiffrée, compressée, coupée en
morceaux), un fichier écrit hors des chemins surveillés, un envoi par le réseau.

## Adaptateur Python

`python/paravent.py` : un fichier, bibliothèque standard seule, à copier dans l'outil ou à importer
depuis un dépôt voisin. Exemple complet : `exemples/outil_python/`.

```python
import paravent

with paravent.Sortie(declaration="declaration.json") as sortie:
    sortie.emettre("contact", 1042, {"prénom": "…", "rôle": "enseignante"})
    sortie.etat("LISTE_TERMINEE", total=1)
```

- Binaire : option `binaire`, sinon la variable `PARAVENT_BIN`, sinon le `PATH`. Absent ou trop
  ancien, l'outil s'arrête sans rien écrire d'autre que le refus.
- `emettre(type, id, champs)` envoie un objet au filtre ; `emettre_reference(type, ref, champs)`
  quand la source attribue ses références. Option `en_json=True` pour la sortie JSON.
- `etat(CODE, compte=entier, …)` annonce un état sans donnée : un code en majuscules et des comptes
  entiers. Les états sortent après les objets et leurs comptes : `ETAT code=<CODE> <nom>=<n>`, ou
  `{"etat": {"code": …}}` en JSON.
- Dans le bloc `with`, `print` et toute écriture sur `sys.stdout` ou `sys.stderr` lèvent
  `EcritureDirecte` ; l'outil sort en 20 même si l'exception est rattrapée. Ce qu'écrivent le code
  natif et les sous-processus sur les sorties 1 et 2 est jeté.
- Une exception non rattrapée ne laisse sortir que son type, son fichier et sa ligne.
- Un refus du filtre (codes 3 à 13) devient le code de sortie de l'outil.

Ce que l'adaptateur ne couvre pas : ce que l'outil écrit hors du bloc `with`, dans un fichier ou
sur le réseau. Le banc anti-fuite sert à le vérifier.

## Message d'erreur

Une seule ligne, sur la sortie d'erreur :

```
PARAVENT_ERREUR code=<CODE> [ligne=<n>] : <message>
```

`ligne` est le numéro de la ligne d'entrée fautive, à partir de 1.

## Codes

| Sortie | Code | Sens | Que faire |
| --- | --- | --- | --- |
| 0 | — | Tout est passé. | — |
| 2 | `USAGE` | Commande ou option incorrecte. | Corriger l'appel. |
| 3 | `DECLARATION_INVALIDE` | Déclaration absente, illisible ou incohérente. | Corriger la déclaration ; le message donne la position. |
| 4 | `CLE_REFUSEE` | Clé absente, trop courte, ou fichier accessible à d'autres. | `chmod 600`, ou créer la clé (ADR 0001). |
| 5 | `LIGNE_ILLISIBLE` | Une ligne n'est pas un objet au format d'échange. | Corriger l'outil qui produit la ligne. |
| 6 | `TYPE_INCONNU` | Le type d'un objet n'est pas déclaré. | Déclarer le type, après relecture humaine. |
| 7 | `CHAMP_SECRET` | L'outil a envoyé un champ secret. | Corriger l'outil : il ne doit pas lire ce champ. |
| 8 | `COLLISION` | Deux objets d'un même type donnent la même référence. | Prévenir un humain ; voir ADR 0001. |
| 9 | `ENTREE_SORTIE` | Lecture ou écriture impossible. | Relancer ; vérifier le tube. |
| 10 | `FUITE` | Le banc a trouvé au moins une valeur piège. | Corriger l'outil ; ne pas le brancher à un agent. |
| 11 | `PLAN_INVALIDE` | Plan du banc absent, illisible, vide, ou piège trop court. | Corriger le plan. |
| 12 | `COMMANDE_INLANCABLE` | Une commande du plan n'a pas pu être lancée. | Corriger le plan ou le chemin. |
| 13 | `REFERENCE_INVALIDE` | Référence fournie par la source mal formée. | Corriger la source. |
| 14 | `COMMANDE_TROP_LONGUE` | Une commande du plan a dépassé le délai. | Corriger l'outil ou allonger `delai_secondes`. |
| 15 | `MODELE_INVALIDE` | Modèle à trous mal écrit, ou champ secret ou non déclaré. | Corriger le modèle ; le message nomme le champ ou la position. |
| 20 | `ECRITURE_DIRECTE` | Un outil Python a écrit sur une sortie sans passer par le filtre. | Corriger l'outil : `emettre` ou `etat`. |
| 21 | `EXCEPTION` | Exception non rattrapée dans un outil Python ; le message donne type et ligne. | Corriger l'outil. |
| 22 | `BINAIRE_ABSENT` | L'adaptateur ne trouve pas le binaire `paravent`. | Installer le binaire ou régler `PARAVENT_BIN`. |
| 23 | `BINAIRE_TROP_ANCIEN` | Le binaire est plus ancien que ce qu'attend l'adaptateur. | Mettre le binaire à jour. |

Un agent qui reçoit `TYPE_INCONNU`, `CHAMP_SECRET` ou `COLLISION` s'arrête et le signale : il ne
modifie pas la déclaration lui-même.
