"""Adaptateur Python de paravent : un outil émet des objets, le filtre écrit la sortie sûre.

Un seul fichier, bibliothèque standard seule : à copier dans l'outil, ou à importer depuis un
dépôt voisin.

    import paravent

    with paravent.Sortie(declaration="declaration.json") as sortie:
        for contact in lire_les_contacts():
            sortie.emettre("contact", contact["id"], contact)
        sortie.etat("LISTE_TERMINEE", total=3)

Tant que le bloc `with` est ouvert :

- `print`, `sys.stdout.write` et `sys.stderr.write` lèvent `EcritureDirecte`, et l'outil sort
  en erreur même si l'exception a été rattrapée ;
- ce qu'écrivent le code natif et les sous-processus sur les sorties 1 et 2 est jeté ;
- une exception non rattrapée ne laisse sortir que son type et sa ligne, jamais son message.

Tout le travail de l'outil se fait dans le bloc. Codes de sortie : `docs/codes.md`.
"""

import json
import os
import re
import shutil
import subprocess
import sys

__all__ = ["Sortie", "EcritureDirecte", "VERSION_MIN"]

#: Plus ancienne version du binaire acceptée.
VERSION_MIN = (0, 1, 0)

SORTIE_ECRITURE_DIRECTE = 20
SORTIE_EXCEPTION = 21
SORTIE_BINAIRE_ABSENT = 22
SORTIE_BINAIRE_TROP_ANCIEN = 23

_CODE_ETAT = re.compile(r"[A-Z][A-Z0-9_]*")
_NOM_COMPTE = re.compile(r"[a-z][a-z0-9_]*")
_NOM_DE_TYPE = re.compile(r"[A-Za-z_][A-Za-z0-9_.]*")
_VERSION = re.compile(r"paravent ([0-9]+)[.]([0-9]+)[.]([0-9]+)")


class EcritureDirecte(Exception):
    """L'outil a écrit sur une sortie sans passer par le filtre."""


class _FiltreArrete(Exception):
    """Le filtre s'est arrêté : son code de sortie devient celui de l'outil."""


class _Refus:
    """Remplace `sys.stdout` et `sys.stderr` : toute écriture est notée, puis refusée."""

    encoding = "utf-8"
    errors = "strict"

    def __init__(self, sortie):
        self._sortie = sortie

    def write(self, _texte):
        self._sortie._ecriture_directe = True
        raise EcritureDirecte("écriture directe refusée : passer par Sortie.emettre")

    def writelines(self, _lignes):
        self.write("")

    def flush(self):
        pass

    def isatty(self):
        return False


def _refuser(code, sortie, message):
    """Écrit un refus sur la vraie sortie d'erreur, puis arrête l'outil."""
    os.write(2, f"PARAVENT_ERREUR code={code} : {message}\n".encode())
    raise SystemExit(sortie)


def _trouver_binaire(indique):
    """Le chemin du binaire, vérifié. Sans binaire convenable, aucun repli : l'outil s'arrête."""
    chemin = indique or os.environ.get("PARAVENT_BIN") or shutil.which("paravent")
    lu = None
    if chemin:
        try:
            lu = subprocess.run(
                [str(chemin), "--version"], capture_output=True, timeout=10, check=False
            )
        except (OSError, subprocess.SubprocessError):
            lu = None
    if lu is None or lu.returncode != 0:
        _refuser(
            "BINAIRE_ABSENT",
            SORTIE_BINAIRE_ABSENT,
            "binaire paravent introuvable (option binaire, PARAVENT_BIN ou PATH)",
        )
    version = _VERSION.fullmatch(lu.stdout.decode("utf-8", "replace").strip())
    if not version or tuple(int(n) for n in version.groups()) < VERSION_MIN:
        attendue = ".".join(str(n) for n in VERSION_MIN)
        _refuser(
            "BINAIRE_TROP_ANCIEN",
            SORTIE_BINAIRE_TROP_ANCIEN,
            f"binaire paravent trop ancien (attendu : {attendue} ou plus)",
        )
    return str(chemin)


class Sortie:
    """La seule sortie d'un outil : des objets vers le filtre, des états sans donnée."""

    def __init__(self, declaration, cle=None, en_json=False, binaire=None):
        self._declaration = str(declaration)
        self._cle = None if cle is None else str(cle)
        self._en_json = en_json
        self._binaire = binaire
        self._etats = []
        self._ecriture_directe = False
        self._filtre = None

    def __enter__(self):
        binaire = _trouver_binaire(self._binaire)
        arguments = [binaire, "filtrer", "--declaration", self._declaration]
        if self._cle:
            arguments += ["--cle", self._cle]
        if self._en_json:
            arguments.append("--json")
        sys.stdout.flush()
        sys.stderr.flush()
        # Les vraies sorties sont mises de côté : seuls le filtre et les refus y écrivent.
        self._fd_sortie = os.dup(1)
        self._fd_erreur = os.dup(2)
        self._filtre = subprocess.Popen(
            arguments, stdin=subprocess.PIPE, stdout=self._fd_sortie, stderr=self._fd_erreur
        )
        nul = os.open(os.devnull, os.O_WRONLY)
        os.dup2(nul, 1)
        os.dup2(nul, 2)
        os.close(nul)
        self._flux = (sys.stdout, sys.stderr)
        sys.stdout = _Refus(self)
        sys.stderr = _Refus(self)
        return self

    def emettre(self, type_objet, identifiant, champs=None):
        """Envoie un objet au filtre. `identifiant` est celui de la source, texte ou entier."""
        self._envoyer({"type": type_objet, "id": identifiant, "champs": dict(champs or {})})

    def emettre_reference(self, type_objet, reference, champs=None):
        """Envoie un objet dont la source a déjà attribué la référence."""
        self._envoyer({"type": type_objet, "ref": reference, "champs": dict(champs or {})})

    def etat(self, code, **comptes):
        """Annonce un état : un code en majuscules et des comptes entiers, rien d'autre.

        Les états sortent après les objets et leurs comptes.
        """
        if not isinstance(code, str) or not _CODE_ETAT.fullmatch(code):
            raise ValueError("code d'état : majuscules, chiffres et « _ »")
        for nom, nombre in comptes.items():
            if not _NOM_COMPTE.fullmatch(nom):
                raise ValueError("nom de compte : minuscules, chiffres et « _ »")
            if isinstance(nombre, bool) or not isinstance(nombre, int):
                raise ValueError("un compte est un entier")
        self._etats.append((code, dict(comptes)))

    def _envoyer(self, objet):
        ligne = json.dumps(objet, ensure_ascii=False, default=str) + "\n"
        try:
            self._filtre.stdin.write(ligne.encode("utf-8"))
            self._filtre.stdin.flush()
        except OSError:
            # Le filtre a refusé une ligne précédente et s'est arrêté.
            raise _FiltreArrete() from None

    def __exit__(self, type_exception, exception, trace):
        try:
            self._filtre.stdin.close()
        except OSError:
            pass
        code_filtre = self._filtre.wait()
        termine = exception is None or (
            isinstance(exception, SystemExit) and exception.code in (None, 0)
        )
        if termine and code_filtre == 0 and not self._ecriture_directe:
            os.write(self._fd_sortie, self._texte_des_etats().encode("utf-8"))
        self._retablir()

        if self._ecriture_directe:
            _refuser(
                "ECRITURE_DIRECTE",
                SORTIE_ECRITURE_DIRECTE,
                "l'outil a écrit sur une sortie sans passer par le filtre",
            )
        if code_filtre != 0:
            # Le filtre a déjà écrit son refus.
            raise SystemExit(code_filtre)
        if termine:
            return isinstance(exception, SystemExit)
        if isinstance(exception, SystemExit) and isinstance(exception.code, int):
            return False
        if isinstance(exception, KeyboardInterrupt):
            raise SystemExit(130)
        nom = type_exception.__name__
        if not _NOM_DE_TYPE.fullmatch(nom):
            nom = "Exception"
        _refuser(
            "EXCEPTION",
            SORTIE_EXCEPTION,
            f"exception non rattrapée dans l'outil, type={nom} lieu={_lieu(trace)}",
        )

    def _texte_des_etats(self):
        lignes = []
        for code, comptes in self._etats:
            if self._en_json:
                lignes.append(json.dumps({"etat": {"code": code, **comptes}}))
            else:
                parties = [f"ETAT code={code}"] + [f"{nom}={n}" for nom, n in comptes.items()]
                lignes.append(" ".join(parties))
        return "".join(ligne + "\n" for ligne in lignes)

    def _retablir(self):
        sys.stdout, sys.stderr = self._flux
        os.dup2(self._fd_sortie, 1)
        os.dup2(self._fd_erreur, 2)
        os.close(self._fd_sortie)
        os.close(self._fd_erreur)


def _lieu(trace):
    """Le fichier et la ligne où l'exception est née : du code, pas des données."""
    if trace is None:
        return "inconnu"
    while trace.tb_next is not None:
        trace = trace.tb_next
    fichier = os.path.basename(trace.tb_frame.f_code.co_filename)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", fichier):
        fichier = "fichier"
    return f"{fichier}:{trace.tb_lineno}"
