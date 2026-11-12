"""Sextet (Act III) sung text per character, as the score has it, once (no repeats),
split into short phrases with a very literal English gloss under each.

Used by phase8_text.py to clean the OCR'd lines (the OCR decides what is sung when and
how often; this only supplies clean spelling) and to give each line its aligned gloss.
Verses are numbered per character in the order they are first sung.

Glosses are deliberately very literal: Italian word order where it still makes sense,
nothing smoothed over, so the English stays close to the sounds heard.
"""

DOLCE = [("Al dolce contento", "To the sweet contentment"), ("di questo momento", "of this moment"),
         ("quest'anima", "this soul"), ("appena", "barely"), ("resister", "to resist"),
         ("or sa.", "now knows how.")]
EFFETTO = [("È un effetto", "It is an effect"), ("di buon core,", "of good heart,"),
           ("tutto amore", "all love"), ("è quel che fa.", "is that which she does.")]
NON_SAPPIAM = [("Non sappiam", "We do not know"), ("com'è la cosa,", "how the thing is,"),
               ("osservate", "observe"), ("un poco là.", "a little there.")]
SUA_MADRE = [("Sua madre!", "His mother!")]
SUO_PADRE = [("Suo padre!", "His father!")]
FIGLIO_AMATO = [("Figlio amato!", "Son beloved!")]

TEXT = {
    "SUSANNA": [
        [("Alto, alto,", "Halt, halt,"), ("signor Conte,", "lord Count,"),
         ("mille doppie", "a thousand doubloons"), ("son qui pronte,", "are here ready,"),
         ("a pagar vengo", "to pay I come"), ("per Figaro,", "for Figaro,"),
         ("ed a porlo", "and to put him"), ("in libertà.", "in freedom.")],
        [("Già d'accordo", "Already in agreement"), ("colla sposa!", "with the bride!"),
         ("Giusti Dei,", "Just gods,"), ("che infedeltà!", "what infidelity!"),
         ("Lascia,", "Let go,"), ("iniquo!", "wicked one!")],
        [("Senti questa!", "Feel this one!")],
        [("Fremo,", "I tremble,"), ("smanio", "I rave"), ("dal furore,", "from the fury,"),
         ("una vecchia", "an old woman"), ("a me la fa.", "plays it on me.")],
        [("Sua madre?", "His mother?")],
        [("Tua madre?", "Your mother?")],
        [("Suo padre?", "His father?")],
        [("Tuo padre?", "Your father?")],
        DOLCE,
    ],
    "MARCELLINA": [
        [("Riconosci", "Recognize"), ("in questo amplesso", "in this embrace"),
         ("una madre,", "a mother,"), ("amato figlio!", "beloved son!")],
        FIGLIO_AMATO,
        EFFETTO,
        [("Lo sdegno", "The anger"), ("calmate,", "calm,"), ("mia cara figliuola,", "my dear little daughter,"),
         ("sua madre", "his mother"), ("abbracciate,", "embrace,"), ("che or", "who now"),
         ("vostra sarà.", "yours will be.")],
        SUA_MADRE,
        SUO_PADRE,
        DOLCE,
    ],
    "DON CURZIO": [
        [("Ei", "He"), ("suo padre?", "his father?"), ("Ella", "She"), ("sua madre?", "his mother?"),
         ("L'imeneo", "The marriage"), ("non può seguir.", "cannot follow.")],
        NON_SAPPIAM,
        [("Freme", "He trembles"), ("e smania", "and raves"), ("dal furore,", "from the fury,"),
         ("il destino", "fate"), ("gliela fa.", "plays it on him.")],
        SUA_MADRE,
        SUO_PADRE,
        [("Al fiero tormento", "To the fierce torment"), ("di questo momento", "of this moment"),
         ("quell'anima", "that soul"), ("appena", "barely"), ("resister", "to resist"),
         ("or sa.", "now knows how.")],
    ],
    "IL CONTE": [
        [("Son smarrito,", "I am lost,"), ("son stordito,", "I am stunned,"),
         ("meglio è assai", "better it is by far"), ("di qua partir.", "from here to leave.")],
        NON_SAPPIAM,
        [("Fremo,", "I tremble,"), ("smanio", "I rave"), ("dal furore,", "from the fury,"),
         ("il destino", "fate"), ("a me la fa.", "plays it on me.")],
        SUA_MADRE,
        SUO_PADRE,
        [("Al fiero tormento", "To the fierce torment"), ("di questo momento", "of this moment"),
         ("quest'anima", "this soul"), ("appena", "barely"), ("resister", "to resist"),
         ("or sa.", "now knows how.")],
    ],
    "DON BARTOLO": [
        [("Resistenza", "Resistance"), ("la coscienza", "the conscience"),
         ("far non lascia", "to put up does not let (me)"), ("al tuo desir.", "to your desire.")],
        FIGLIO_AMATO,
        EFFETTO,
        SUA_MADRE,
        SUO_PADRE,
        DOLCE,
    ],
    "FIGARO": [
        [("Padre mio,", "Father mine,"), ("fate lo stesso,", "do the same,"),
         ("non mi fate", "do not make me"), ("più arrossir.", "blush any more.")],
        [("Parenti amati!", "Parents beloved!")],
        [("No,", "No,"), ("t'arresta!", "stop yourself!")],
        [("Senti,", "Listen,"), ("oh cara!", "oh dear!")],
        EFFETTO,
        [("E quello", "And that one"), ("è mio padre,", "is my father,"),
         ("che a te", "who to you"), ("lo dirà.", "it will say.")],
        [("E quella", "And that one"), ("è mia madre,", "is my mother,"),
         ("che a te", "who to you"), ("lo dirà.", "it will say.")],
        DOLCE,
    ],
}
