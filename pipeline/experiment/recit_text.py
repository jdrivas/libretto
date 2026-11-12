"""Sung text of the recitatives after the Sextet, with a very literal phrase-aligned gloss.

Each speech: (character, [(italian phrase, literal english), ...]).
"Eccovi, o caro amico" follows the Ricordi vocal score (No. 41, PDF pp. 236-237), which
differs from the murashev libretto in places ("al gioir mio", Susanna's repeated "Chi al
par di me contenta?", spelling). "Andiam, bel paggio" is not in that part of the Ricordi
score; its text follows the libretto, corrected ("Andiam, andiam", "ragazze", "se ti trova").

ENSEMBLE marks speeches sung together: same words, same time, several characters.
"""

ENSEMBLE = "ENSEMBLE"

ECCOVI = [
    ("MARCELLINA", [("Eccovi,", "Here you have,"), ("o caro amico,", "o dear friend,"),
                    ("il dolce frutto", "the sweet fruit"), ("dell'antico amor nostro...", "of-the old love of ours...")]),
    ("DON BARTOLO", [("Or non parliamo", "Now not let's speak"), ("di fatti sì remoti:", "of deeds so distant:"),
                     ("egli è mio figlio,", "he is my son,"), ("mia consorte voi siete;", "my wife you are;"),
                     ("e le nozze farem", "and the wedding we'll make"), ("quando volete.", "when you wish.")]),
    ("MARCELLINA", [("Oggi,", "Today,"), ("e doppie saranno;", "and double they will be;"),
                    ("prendi,", "take,"), ("questo è il biglietto", "this is the note"),
                    ("del danar che a me devi,", "of-the money that to me you owe,"),
                    ("ed è tua dote.", "and it is your dowry.")]),
    ("SUSANNA", [("Prendi ancor", "Take also"), ("questa borsa.", "this purse.")]),
    ("DON BARTOLO", [("E questa ancora.", "And this one too.")]),
    ("FIGARO", [("Bravi;", "Well done;"), ("gittate pur,", "throw away then,"), ("ch'io piglio ognora.", "for I take always.")]),
    ("SUSANNA", [("Voliamo ad informar", "Let us fly to inform"), ("d'ogni avventura", "of every adventure"),
                 ("Madama e nostro zio.", "Madam and our uncle."),
                 ("Chi al par di me", "Who equal to me"), ("contenta?", "happy?"),
                 ("Chi al par di me", "Who equal to me"), ("contenta?", "happy?")]),
    ("FIGARO", [("Io.", "I.")]),
    ("DON BARTOLO", [("Io.", "I.")]),
    ("MARCELLINA", [("Io.", "I.")]),
    (ENSEMBLE, ["SUSANNA", "MARCELLINA", "DON BARTOLO", "FIGARO"],
     [("E schiatti", "And may burst"), ("il signor Conte", "the lord Count"), ("al gioir mio.", "at-the joy mine.")]),
]

ANDIAM = [
    ("BARBARINA", [("Andiam, andiam,", "Let's go, let's go,"), ("bel paggio:", "handsome page:"),
                   ("in casa mia", "in house mine"), ("tutte ritroverai", "all you will find"),
                   ("le più belle ragazze", "the most beautiful girls"), ("del castello:", "of-the castle:"),
                   ("di tutte sarai tu", "of all will be you"), ("certo il più bello.", "surely the most beautiful.")]),
    ("CHERUBINO", [("Ah! se il Conte mi trova,", "Ah! if the Count me finds,"), ("misero me!", "wretched me!"),
                   ("Tu sai", "You know"), ("che partito ei mi crede", "that departed he me believes"),
                   ("per Siviglia.", "for Seville.")]),
    ("BARBARINA", [("Oh ve' che maraviglia!", "Oh look what a marvel!"), ("E se ti trova,", "And if you he finds,"),
                   ("non sarà cosa nuova.", "not will be thing new."), ("Odi,", "Listen,"),
                   ("vogliam vestirti", "we want to dress you"), ("come noi:", "like us:"),
                   ("tutte insieme", "all together"), ("andrem poi", "we'll go then"),
                   ("a presentar de' fiori", "to present some flowers"), ("a Madamina.", "to little Madam."),
                   ("Fidati,", "Trust,"), ("o Cherubin,", "o Cherubino,"), ("di Barbarina.", "in Barbarina.")]),
]

# Score order for ensemble rows (continues the Sextet's cast order).
VOICE = {"SUSANNA": 0, "MARCELLINA": 1, "DON CURZIO": 2, "IL CONTE": 3, "DON BARTOLO": 4, "FIGARO": 5,
         "BARBARINA": 6, "CHERUBINO": 7, "LA CONTESSA": 8}
