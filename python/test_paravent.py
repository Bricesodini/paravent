"""Tests de l'adaptateur Python, contre le vrai binaire.

    cargo build && python3 -m unittest discover -s python -v

Le binaire se prend dans PARAVENT_BIN, sinon dans target/debug/.
"""

import os
import pathlib
import shutil
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest

ICI = pathlib.Path(__file__).resolve().parent
BINAIRE = os.environ.get("PARAVENT_BIN") or str(ICI.parent / "target" / "debug" / "paravent")

DECLARATION = """{
  "version": 1,
  "types": {
    "contact": {
      "publics": ["rôle", "état"],
      "personnels": ["prénom", "courriel"],
      "secrets": ["jeton"],
      "comptes": ["état"]
    }
  }
}"""

PERSONNEL = "Camille-VALEUR-TEMOIN"

EN_TETE = f"""
import os, subprocess, sys
import paravent
PERSONNEL = {PERSONNEL!r}
CONTACT = {{"prénom": PERSONNEL, "courriel": "", "rôle": "enseignante", "état": "actif"}}
"""


class Adaptateur(unittest.TestCase):
    def setUp(self):
        self.assertTrue(os.path.exists(BINAIRE), "binaire absent : lancer cargo build")
        self.dossier = pathlib.Path(tempfile.mkdtemp(prefix="paravent-python-"))
        self.addCleanup(shutil.rmtree, self.dossier, True)
        (self.dossier / "declaration.json").write_text(DECLARATION, encoding="utf-8")
        cle = self.dossier / "cle"
        cle.write_bytes(bytes([42]) * 32)
        cle.chmod(0o600)
        shutil.copy(ICI / "paravent.py", self.dossier / "paravent.py")

    def lancer(self, corps, binaire=BINAIRE):
        """Joue un outil d'une page et rend (code, sortie, erreur)."""
        script = self.dossier / "outil.py"
        script.write_text(EN_TETE + textwrap.dedent(corps), encoding="utf-8")
        environnement = dict(os.environ, PARAVENT_CLE=str(self.dossier / "cle"))
        environnement.pop("PARAVENT_BIN", None)
        if binaire is not None:
            environnement["PARAVENT_BIN"] = str(binaire)
        else:
            environnement["PATH"] = str(self.dossier)
        r = subprocess.run(
            [sys.executable, str(script)],
            cwd=self.dossier,
            env=environnement,
            capture_output=True,
            timeout=60,
            check=False,
        )
        sortie, erreur = r.stdout.decode(), r.stderr.decode()
        self.assertNotIn(PERSONNEL, sortie, "une valeur personnelle est sortie")
        self.assertNotIn(PERSONNEL, erreur, "une valeur personnelle est sortie en erreur")
        return r.returncode, sortie, erreur

    def test_emet_des_objets_et_des_etats(self):
        code, sortie, erreur = self.lancer(
            """
            with paravent.Sortie("declaration.json") as s:
                s.emettre("contact", 1042, CONTACT)
                s.emettre("contact", "1043", dict(CONTACT, état="parti"))
                s.etat("LISTE_TERMINEE", total=2)
            """
        )
        self.assertEqual((code, erreur), (0, ""))
        lignes = sortie.splitlines()
        self.assertEqual(len(lignes), 7, sortie)
        self.assertTrue(lignes[0].startswith("contact-"))
        self.assertIn("prénom:renseigné", lignes[0])
        self.assertIn("courriel:vide", lignes[0])
        self.assertEqual(lignes[2:5], ["# comptes", "contact\t2", "contact.état=actif\t1"])
        self.assertEqual(lignes[6], "ETAT code=LISTE_TERMINEE total=2")

    def test_emet_du_json_et_une_reference_fournie(self):
        code, sortie, _ = self.lancer(
            """
            with paravent.Sortie("declaration.json", en_json=True) as s:
                s.emettre_reference("contact", "contact-k3v9x2", CONTACT)
                s.etat("FAIT", total=1)
            """
        )
        self.assertEqual(code, 0)
        lignes = sortie.splitlines()
        self.assertIn('"ref":"contact-k3v9x2"', lignes[0])
        self.assertEqual(lignes[2], '{"etat": {"code": "FAIT", "total": 1}}')

    def test_les_sorties_reviennent_apres_le_bloc(self):
        code, sortie, _ = self.lancer(
            """
            with paravent.Sortie("declaration.json") as s:
                pass
            print("après le bloc")
            """
        )
        self.assertEqual(code, 0)
        self.assertTrue(sortie.endswith("après le bloc\n"))

    def test_une_ecriture_directe_est_refusee(self):
        for ecriture in ("print(PERSONNEL)", "sys.stdout.write(PERSONNEL)", "sys.stderr.write(PERSONNEL)"):
            with self.subTest(ecriture):
                code, sortie, erreur = self.lancer(
                    f"""
                    with paravent.Sortie("declaration.json") as s:
                        s.emettre("contact", 1, CONTACT)
                        {ecriture}
                    """
                )
                self.assertEqual(code, 20)
                self.assertTrue(erreur.startswith("PARAVENT_ERREUR code=ECRITURE_DIRECTE"), erreur)
                self.assertNotIn("ETAT", sortie)

    def test_une_ecriture_directe_rattrapee_fait_quand_meme_echouer(self):
        code, _, erreur = self.lancer(
            """
            with paravent.Sortie("declaration.json") as s:
                try:
                    print(PERSONNEL)
                except Exception:
                    pass
                s.etat("FAIT")
            """
        )
        self.assertEqual(code, 20)
        self.assertIn("code=ECRITURE_DIRECTE", erreur)

    def test_le_code_natif_et_les_sous_processus_n_ecrivent_nulle_part(self):
        code, sortie, erreur = self.lancer(
            """
            with paravent.Sortie("declaration.json") as s:
                os.write(1, PERSONNEL.encode())
                os.write(2, PERSONNEL.encode())
                subprocess.run(["sh", "-c", f"echo {PERSONNEL}; echo {PERSONNEL} >&2"], check=True)
                s.etat("FAIT")
            """
        )
        self.assertEqual((code, erreur), (0, ""))
        self.assertEqual(sortie, "# comptes\nETAT code=FAIT\n")

    def test_une_exception_ne_laisse_sortir_que_son_type_et_sa_ligne(self):
        code, sortie, erreur = self.lancer(
            """
            def chercher():
                raise ValueError(f"contact introuvable : {PERSONNEL}")
            with paravent.Sortie("declaration.json") as s:
                s.emettre("contact", 1, CONTACT)
                chercher()
            """
        )
        self.assertEqual(code, 21)
        self.assertRegex(
            erreur,
            r"^PARAVENT_ERREUR code=EXCEPTION : .* type=ValueError lieu=outil\.py:\d+\n$",
        )
        self.assertNotIn("Traceback", erreur)
        self.assertNotIn("# comptes", sortie.replace("# comptes\n", "", 1))

    def test_un_exit_garde_son_code_sans_message(self):
        code, _, erreur = self.lancer(
            """
            with paravent.Sortie("declaration.json") as s:
                sys.exit(3)
            """
        )
        self.assertEqual((code, erreur), (3, ""))
        code, _, erreur = self.lancer(
            """
            with paravent.Sortie("declaration.json") as s:
                sys.exit(PERSONNEL)
            """
        )
        self.assertEqual(code, 21)

    def test_le_refus_du_filtre_devient_celui_de_l_outil(self):
        code, _, erreur = self.lancer(
            """
            with paravent.Sortie("declaration.json") as s:
                s.emettre("contact", 1, {"jeton": PERSONNEL})
                for i in range(2000):
                    s.emettre("contact", i + 2, CONTACT)
            """
        )
        self.assertEqual(code, 7)
        self.assertTrue(erreur.startswith("PARAVENT_ERREUR code=CHAMP_SECRET ligne=1"), erreur)

    def test_un_etat_ne_porte_que_des_comptes(self):
        for appel in (
            's.etat("fait")',
            "s.etat(PERSONNEL)",
            's.etat("FAIT", nom=PERSONNEL)',
            's.etat("FAIT", total=1.5)',
            's.etat("FAIT", total=True)',
            's.etat("FAIT", **{PERSONNEL: 1})',
        ):
            with self.subTest(appel):
                code, sortie, erreur = self.lancer(
                    f"""
                    with paravent.Sortie("declaration.json") as s:
                        {appel}
                    """
                )
                self.assertEqual(code, 21)
                self.assertIn("type=ValueError", erreur)
                self.assertNotIn("ETAT", sortie)

    def test_sans_binaire_aucun_repli_en_clair(self):
        corps = """
            with paravent.Sortie("declaration.json") as s:
                s.emettre("contact", 1, CONTACT)
            print(PERSONNEL)
            """
        for binaire in (None, self.dossier / "absent"):
            with self.subTest(binaire):
                code, sortie, erreur = self.lancer(corps, binaire=binaire)
                self.assertEqual((code, sortie), (22, ""))
                self.assertTrue(erreur.startswith("PARAVENT_ERREUR code=BINAIRE_ABSENT"), erreur)

    def test_un_binaire_trop_ancien_est_refuse(self):
        for annonce in ("paravent 0.0.9", "autre chose"):
            with self.subTest(annonce):
                ancien = self.dossier / "ancien"
                ancien.write_text(f"#!/bin/sh\necho '{annonce}'\n", encoding="utf-8")
                ancien.chmod(ancien.stat().st_mode | stat.S_IXUSR)
                code, sortie, erreur = self.lancer(
                    """
                    with paravent.Sortie("declaration.json") as s:
                        s.emettre("contact", 1, CONTACT)
                    """,
                    binaire=ancien,
                )
                self.assertEqual((code, sortie), (23, ""))
                self.assertTrue(
                    erreur.startswith("PARAVENT_ERREUR code=BINAIRE_TROP_ANCIEN"), erreur
                )


if __name__ == "__main__":
    unittest.main()
