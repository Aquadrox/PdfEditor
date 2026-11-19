#!/usr/bin/env python3
"""Génère les PDF de test de pde-pdf (fichiers committés à côté de ce script).

Usage : python3 make_fixtures.py
  - simple.pdf    : écrit à la main, sans dépendance ;
  - form.pdf      : formulaire écrit à la main (texte, case, radios, liste) ;
  - form-smart.pdf : formulaire « intelligent » écrit à la main (date,
                    montant, total calculé, liste modifiable, liste à choix
                    multiples, mot de passe, champ obligatoire, signature,
                    bouton poussoir, nom à deux niveaux) ;
  - a4.pdf        : vrai format A4 (595,2756 × 841,8898 pt, dimensions non
                    entières comme chez LibreOffice ou Word) ;
  - encrypted.pdf : simple.pdf chiffré en AES-256, nécessite `pypdf` et
                    `cryptography` (pip install pypdf cryptography).

Les contenus sont fictifs ; le mot de passe de test n'est pas un secret.
simple.pdf est reproductible ; encrypted.pdf change à chaque génération
(sel et vecteurs d'initialisation aléatoires) : ne le régénérer que si besoin.
"""

from pathlib import Path

HERE = Path(__file__).resolve().parent
# Mot de passe du PDF chiffré de test (repris par les tests Rust).
TEST_PASSWORD = "mot-de-passe-de-test"


def text_string(value: str) -> bytes:
    """Chaîne de texte PDF en UTF-16BE avec BOM (accents sûrs)."""
    return b"<FEFF" + value.encode("utf-16-be").hex().upper().encode() + b">"


def stream(content: bytes) -> bytes:
    return b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream"


def build_simple() -> bytes:
    page1 = (
        b"BT /F1 24 Tf 72 760 Td (Bonjour PdfEditor) Tj ET\n"
        # Rectangle noir plein : zone sombre connue pour les tests de rendu.
        b"0 g 72 600 200 100 re f\n"
    )
    page2 = b"BT /F1 18 Tf 72 760 Td (Page deux) Tj ET\n"
    objects = {
        1: b"<< /Type /Catalog /Pages 2 0 R /Outlines 8 0 R /PageMode /UseOutlines >>",
        2: b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] "
        b"/Resources << /Font << /F1 7 0 R >> >> /Contents 5 0 R >>",
        # Page 2 tournée d'un quart de tour : affichée en paysage (842 × 595).
        4: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Rotate 90 "
        b"/Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>",
        5: stream(page1),
        6: stream(page2),
        7: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        8: b"<< /Type /Outlines /First 9 0 R /Last 10 0 R /Count 3 >>",
        9: b"<< /Title " + text_string("Introduction") + b" /Parent 8 0 R /Next 10 0 R "
        b"/Dest [3 0 R /Fit] >>",
        10: b"<< /Title " + text_string("Annexe") + b" /Parent 8 0 R /Prev 9 0 R "
        b"/First 11 0 R /Last 11 0 R /Count 1 /Dest [4 0 R /Fit] >>",
        11: b"<< /Title " + text_string("Détail") + b" /Parent 10 0 R /Dest [4 0 R /Fit] >>",
        12: b"<< /Title " + text_string("Document de test") + b" /Producer (make_fixtures.py) >>",
    }
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = {}
    for number in sorted(objects):
        offsets[number] = len(out)
        out += b"%d 0 obj\n" % number + objects[number] + b"\nendobj\n"
    xref = len(out)
    count = max(objects) + 1
    out += b"xref\n0 %d\n0000000000 65535 f \n" % count
    for number in range(1, count):
        out += b"%010d 00000 n \n" % offsets[number]
    out += b"trailer\n<< /Size %d /Root 1 0 R /Info 12 0 R >>\n" % count
    out += b"startxref\n%d\n%%%%EOF\n" % xref
    return bytes(out)


def write_pdf(objects: dict, info: int) -> bytes:
    """Objets numérotés → fichier PDF (table xref classique)."""
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = {}
    for number in sorted(objects):
        offsets[number] = len(out)
        out += b"%d 0 obj\n" % number + objects[number] + b"\nendobj\n"
    xref = len(out)
    count = max(objects) + 1
    out += b"xref\n0 %d\n0000000000 65535 f \n" % count
    for number in range(1, count):
        out += b"%010d 00000 n \n" % offsets[number]
    out += b"trailer\n<< /Size %d /Root 1 0 R /Info %d 0 R >>\n" % (count, info)
    out += b"startxref\n%d\n%%%%EOF\n" % xref
    return bytes(out)


def build_form() -> bytes:
    """Formulaire : champ texte « nom » (avec une apparence déjà là), case
    « accord », boutons radio « civilite » (M, Mme), liste « ville »."""
    page = (
        b"BT /F1 18 Tf 72 760 Td (Formulaire de test) Tj ET\n"
        b"BT /F1 11 Tf 72 705 Td (Nom :) Tj ET\n"
    )
    old_value = b"/Tx BMC q BT /Helv 12 Tf 0 g 2 6 Td (Ancien) Tj ET Q EMC"
    box = b"q 0 g 3 3 9 9 re f Q"
    dot = b"q 0 g 4 4 7 7 re f Q"
    empty = b""
    radio = lambda rect, state, ap_on: (
        b"<< /Type /Annot /Subtype /Widget /Parent 9 0 R /Rect [%s] /P 3 0 R /F 4 "
        b"/AS /Off /MK << /BC [0 0 0] >> /AP << /N << /%s %d 0 R /Off 15 0 R >> >> >>"
        % (rect, state, ap_on)
    )
    objects = {
        1: b"<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [6 0 R 7 0 R 9 0 R 12 0 R] "
        b"/DR << /Font << /Helv 5 0 R >> >> /DA (/Helv 0 Tf 0 g) >> >>",
        2: b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> "
        b"/Contents 4 0 R /Annots [6 0 R 7 0 R 10 0 R 11 0 R 12 0 R] >>",
        4: stream(page),
        5: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        6: b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (nom) /V (Ancien) /Rect [120 700 320 722] /P 3 0 R "
        b"/DA (/Helv 12 Tf 0 g) /F 4 /AP << /N 8 0 R >> >>",
        7: b"<< /Type /Annot /Subtype /Widget /FT /Btn /T (accord) /Rect [120 650 135 665] /P 3 0 R "
        b"/V /Off /AS /Off /F 4 /MK << /BC [0 0 0] >> /AP << /N << /Oui 13 0 R /Off 15 0 R >> >> >>",
        8: b"<< /Type /XObject /Subtype /Form /BBox [0 0 200 22] /Resources << /Font << /Helv 5 0 R >> >> "
        b"/Length %d >>\nstream\n" % len(old_value) + old_value + b"\nendstream",
        9: b"<< /FT /Btn /Ff 49152 /T (civilite) /V /Off /Kids [10 0 R 11 0 R] >>",
        10: radio(b"120 600 135 615", b"M", 14),
        11: radio(b"160 600 175 615", b"Mme", 14),
        12: b"<< /Type /Annot /Subtype /Widget /FT /Ch /Ff 131072 /T (ville) /Opt [(Paris) (Lyon)] /V (Paris) "
        b"/Rect [120 550 260 568] /P 3 0 R /DA (/Helv 11 Tf 0 g) /F 4 >>",
        13: b"<< /Type /XObject /Subtype /Form /BBox [0 0 15 15] /Length %d >>\nstream\n" % len(box) + box + b"\nendstream",
        14: b"<< /Type /XObject /Subtype /Form /BBox [0 0 15 15] /Length %d >>\nstream\n" % len(dot) + dot + b"\nendstream",
        15: b"<< /Type /XObject /Subtype /Form /BBox [0 0 15 15] /Length 0 >>\nstream\n" + empty + b"\nendstream",
        16: b"<< /Title " + text_string("Formulaire de test") + b" /Author (Test) >>",
    }
    return write_pdf(objects, 16)


def js(script: str) -> bytes:
    """Action JavaScript (texte seulement : PdfEditor ne l'exécute jamais)."""
    return b"<< /S /JavaScript /JS (" + script.encode() + b") >>"


def build_smart_form() -> bytes:
    """Champs reconnus par leurs scripts d'Acrobat et leurs drapeaux."""
    page = b"BT /F1 18 Tf 72 760 Td (Formulaire intelligent) Tj ET\n"
    widget = lambda name, rect, extra: (
        b"<< /Type /Annot /Subtype /Widget /T (" + name + b") /Rect [" + rect + b"] /P 3 0 R /F 4 "
        b"/DA (/Helv 10 Tf 0 g) " + extra + b" >>"
    )
    objects = {
        1: b"<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [6 0 R 7 0 R 8 0 R 9 0 R 10 0 R 11 0 R "
        b"12 0 R 13 0 R 14 0 R 15 0 R] /DR << /Font << /Helv 5 0 R >> >> /DA (/Helv 0 Tf 0 g) /CO [9 0 R] >> >>",
        2: b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> "
        b"/Contents 4 0 R /Annots [6 0 R 7 0 R 8 0 R 9 0 R 10 0 R 11 0 R 12 0 R 13 0 R 14 0 R 16 0 R] >>",
        4: stream(page),
        5: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        6: widget(b"date", b"72 700 232 718", b"/FT /Tx /Ff 2 /TU (Date de naissance) /Q 1 "
           b"/MK << /BC [0 0 1] /BG [1 1 0.8] >> /AA << /F " + js('AFDate_FormatEx("dd/mm/yyyy");')
           + b" /K " + js('AFDate_KeystrokeEx("dd/mm/yyyy");') + b" >>"),
        7: widget(b"prix", b"72 670 232 688", b"/FT /Tx /AA << /F " + js('AFNumber_Format(2, 3, 0, 0, " \\240", false);')
           + b" >>"),
        8: widget(b"quantite", b"250 670 330 688", b"/FT /Tx /V (3)"),
        9: widget(b"total", b"72 640 232 658", b"/FT /Tx /Ff 1 /AA << /C "
           + js('AFSimple_Calculate("SUM", new Array("prix", "quantite"));') + b" >>"),
        10: widget(b"pays", b"72 610 232 628", b"/FT /Ch /Ff 393216 /Opt [(France) (Belgique)] /V (Suisse)"),
        11: widget(b"langues", b"72 540 232 600", b"/FT /Ch /Ff 2097152 /Opt [(fr) (en) (de)] /V [(fr) (de)]"),
        12: widget(b"secret", b"72 510 232 528", b"/FT /Tx /Ff 8192"),
        13: widget(b"signature", b"300 450 500 500", b"/FT /Sig"),
        14: widget(b"imprimer", b"72 450 150 470", b"/FT /Btn /Ff 65536"),
        15: b"<< /T (client) /Kids [16 0 R] >>",
        16: b"<< /Type /Annot /Subtype /Widget /Parent 15 0 R /T (ville) /FT /Tx /Rect [72 420 232 438] "
        b"/P 3 0 R /F 4 /DA (/Helv 10 Tf 0 g) /MaxLen 30 >>",
        17: b"<< /Title " + text_string("Formulaire intelligent") + b" >>",
    }
    return write_pdf(objects, 17)


def build_a4() -> bytes:
    """Page A4 aux dimensions exactes, non entières : PDFium arrondit ses
    pixels et la page ne doit pas pour autant passer pour tournée."""
    page = (
        b"BT /F1 12 Tf 72 760 Td (Premier paragraphe du bail.) Tj ET\n"
        b"BT /F1 12 Tf 72 740 Td (Second paragraphe du bail.) Tj ET\n"
    )
    objects = {
        1: b"<< /Type /Catalog /Pages 2 0 R >>",
        2: b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.2756 841.8898] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        4: stream(page),
        5: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        6: b"<< /Title " + text_string("A4 de test") + b" /Producer (make_fixtures.py) >>",
    }
    return write_pdf(objects, 6)


def build_encrypted(simple: Path) -> bytes:
    import io

    from pypdf import PdfReader, PdfWriter

    writer = PdfWriter(clone_from=PdfReader(simple))
    writer.encrypt(user_password=TEST_PASSWORD, owner_password=None, algorithm="AES-256")
    buffer = io.BytesIO()
    writer.write(buffer)
    return buffer.getvalue()


def main() -> None:
    simple = HERE / "simple.pdf"
    simple.write_bytes(build_simple())
    print(f"écrit : {simple.name}")
    (HERE / "form.pdf").write_bytes(build_form())
    print("écrit : form.pdf")
    (HERE / "form-smart.pdf").write_bytes(build_smart_form())
    print("écrit : form-smart.pdf")
    (HERE / "a4.pdf").write_bytes(build_a4())
    print("écrit : a4.pdf")
    try:
        (HERE / "encrypted.pdf").write_bytes(build_encrypted(simple))
        print("écrit : encrypted.pdf")
    except ImportError:
        print("pypdf / cryptography absents : encrypted.pdf non régénéré")


if __name__ == "__main__":
    main()
