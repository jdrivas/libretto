"""Sung text of No. 12 "Voi che sapete" and the recitative "Bravo! che bella voce!" (Act II),
from the Ricordi vocal score (Nos. 24-25, printed pp. 98-103), with repeats as sung.
Each speech: (character, [(italian phrase, literal english), ...]); the aria is split into
its stanzas (one speech each) so lines follow the verse."""

A = [("Voi che sapete", "You who know"), ("che cosa è amor,", "what thing is love,"),
     ("donne, vedete", "ladies, see"), ("s'io l'ho nel cor,", "if I it have in-the heart,")]
DONNE = [("donne, vedete", "ladies, see"), ("s'io l'ho nel cor.", "if I it have in-the heart.")]

VOI_CHE_SAPETE = [
    ("CHERUBINO", A + DONNE),
    ("CHERUBINO", [("Quello ch'io provo", "That which I feel"), ("vi ridirò,", "to you I will retell,"),
                   ("è per me nuovo,", "it is for me new,"), ("capir nol so.", "understand it-not I can.")]),
    ("CHERUBINO", [("Sento un affetto", "I feel an affection"), ("pien di desir,", "full of desire,"),
                   ("ch'ora è diletto,", "that now is delight,"), ("ch'ora è martir.", "that now is torment.")]),
    ("CHERUBINO", [("Gelo e poi sento", "I freeze and then I feel"), ("l'alma avvampar,", "the soul blaze up,"),
                   ("e in un momento", "and in a moment"), ("torno a gelar.", "I return to freezing.")]),
    ("CHERUBINO", [("Ricerco un bene", "I seek a good"), ("fuori di me,", "outside of me,"),
                   ("non so chi il tiene,", "not I know who it holds,"), ("non so cos'è;", "not I know what it is;")]),
    ("CHERUBINO", [("sospiro e gemo", "I sigh and I moan"), ("senza voler,", "without wanting,"),
                   ("palpito e tremo", "I throb and I tremble"), ("senza saper;", "without knowing;")]),
    ("CHERUBINO", [("non trovo pace", "not I find peace"), ("notte né dì,", "night nor day,"),
                   ("ma pur mi piace", "but yet to-me it pleases"), ("languir così.", "to languish thus.")]),
    ("CHERUBINO", A + DONNE[:1] + [("s'io l'ho nel cor,", "if I it have in-the heart,")] + DONNE),
]

BRAVO = [
    ("LA CONTESSA", [("Bravo!", "Bravo!"), ("che bella voce!", "what beautiful voice!"),
                     ("io non sapea", "I not knew"), ("che cantaste sì bene.", "that you sang so well.")]),
    ("SUSANNA", [("Oh in verità", "Oh in truth"), ("egli fa tutto ben", "he does everything well"),
                 ("quello ch'ei fa.", "that which he does."), ("Presto, a noi,", "Quick, to us,"),
                 ("bel soldato.", "handsome soldier."), ("Figaro v'informò...", "Figaro you informed...")]),
    ("CHERUBINO", [("Tutto mi disse.", "Everything to-me he told.")]),
    ("SUSANNA", [("Lasciatemi veder:", "Let me see:"), ("andrà benissimo:", "it will go very well:"),
                 ("siam d'eguale statura...", "we are of equal height..."), ("Giù quel manto.", "Off that cloak.")]),
    ("LA CONTESSA", [("Che fai?", "What are you doing?")]),
    ("SUSANNA", [("Niente paura.", "No fear.")]),
    ("LA CONTESSA", [("E se qualcuno entrasse?", "And if someone came in?")]),
    ("SUSANNA", [("Entri,", "Let him come in,"), ("che mal facciamo?", "what harm do we do?"),
                 ("La porta chiuderò.", "The door I will close."), ("Ma come poi", "But how then"),
                 ("acconciargli i capelli?", "to arrange for-him the hair?")]),
    ("LA CONTESSA", [("Una mia cuffia", "A cap of mine"), ("prendi nel gabinetto,", "take in-the closet,"),
                     ("presto.", "quickly."), ("Che carta è quella?", "What paper is that?")]),
    ("CHERUBINO", [("La patente.", "The commission.")]),
    ("LA CONTESSA", [("Che sollecita gente!", "What prompt people!")]),
    ("CHERUBINO", [("L'ebbi or or", "I had it just now"), ("da Basilio.", "from Basilio.")]),
    ("LA CONTESSA", [("Dalla fretta", "From the haste"), ("obbliato hanno il sigillo.", "forgotten they have the seal.")]),
    ("SUSANNA", [("Il sigillo di che?", "The seal of what?")]),
    ("LA CONTESSA", [("Della patente.", "Of the commission.")]),
    ("SUSANNA", [("Cospetto!", "Good heavens!"), ("che premura!", "what a hurry!"), ("Ecco la cuffia.", "Here is the cap.")]),
    ("LA CONTESSA", [("Spicciati:", "Hurry up:"), ("va bene:", "it goes well:"),
                     ("miserabili noi,", "wretched us,"), ("se il Conte viene!", "if the Count comes!")]),
]
