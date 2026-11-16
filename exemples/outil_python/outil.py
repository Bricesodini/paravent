#!/usr/bin/env python3
"""Outil d'exemple branché sur paravent : un carnet de contacts fictif.

    python3 outil.py lister
    python3 outil.py chercher <terme>
    python3 outil.py planter

Il lit ses données en clair et n'écrit que par `paravent.Sortie`. Le banc le vérifie :

    PARAVENT_BIN=… PARAVENT_CLE=… paravent banc --plan plan.json
"""

import json
import pathlib
import sys

ICI = pathlib.Path(__file__).resolve().parent
# L'adaptateur : copié à côté de l'outil, ou pris dans le dépôt voisin.
sys.path[:0] = [str(ICI), str(ICI.parent.parent / "python")]

import paravent  # noqa: E402


def contacts():
    with open(ICI / "donnees.jsonl", encoding="utf-8") as fichier:
        return [json.loads(ligne) for ligne in fichier if ligne.strip()]


def journaliser(commande, total):
    """Le journal ne porte que des comptes (garantie 6)."""
    with open(ICI / "outil.log", "a", encoding="utf-8") as journal:
        journal.write(f"{commande} total={total}\n")


def lister(sortie, _arguments):
    tous = contacts()
    for contact in tous:
        sortie.emettre("contact", contact["id"], contact["champs"])
    journaliser("lister", len(tous))
    sortie.etat("LISTE_TERMINEE", total=len(tous))


def chercher(sortie, arguments):
    """Résolution : du terme donné par l'humain aux références."""
    terme = arguments[0].casefold()
    trouves = [
        c
        for c in contacts()
        if any(terme in str(c["champs"].get(champ, "")).casefold() for champ in ("prénom", "nom"))
    ]
    for contact in trouves:
        sortie.emettre("contact", contact["id"], contact["champs"])
    journaliser("chercher", len(trouves))
    sortie.etat("AUCUN_RESULTAT" if not trouves else "RECHERCHE_TERMINEE", total=len(trouves))


def planter(_sortie, _arguments):
    """Une panne dont le message contient une valeur personnelle."""
    contact = contacts()[0]["champs"]
    raise RuntimeError(f"envoi impossible à {contact['courriel']}")


COMMANDES = {"lister": (lister, 0), "chercher": (chercher, 1), "planter": (planter, 0)}


def principal(arguments):
    with paravent.Sortie(declaration=ICI / "declaration.json") as sortie:
        if not arguments or arguments[0] not in COMMANDES:
            sortie.etat("COMMANDE_INCONNUE")
            return 2
        fonction, attendus = COMMANDES[arguments[0]]
        if len(arguments) - 1 != attendus:
            sortie.etat("ARGUMENTS_INCORRECTS", attendus=attendus)
            return 2
        fonction(sortie, arguments[1:])
    return 0


if __name__ == "__main__":
    sys.exit(principal(sys.argv[1:]))
