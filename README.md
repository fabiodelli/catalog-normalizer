# catalog-normalizer

Prende righe prodotto scritte da esseri umani in fonti diverse e produce un
catalogo strutturato — più un registro di tutto ciò che ha rifiutato, con il
motivo, riga per riga.

Il principio è uno solo: **le regole risolvono, il modello vede solo il residuo,
e le regole validano anche quello.**

```
CSV disordinato ──▶ regole deterministiche ──┬──▶ catalogo normalizzato
                                             │
                                    residuo ambiguo
                                             │
                                      (opzionale) modello
                                             │
                                    ri-validazione con le stesse regole
                                             │
                                             └──▶ log degli scarti, con motivazione
```

Ogni campo che le regole risolvono è un campo che non costa una chiamata di
inferenza. Ogni valore che il modello propone deve superare la stessa regola che
aveva fallito: un colore fuori tavolozza o un EAN con la cifra di controllo
sbagliata viene rifiutato, chiunque lo abbia scritto. Non c'è modo per
un'allucinazione di entrare nel catalogo.

## Provalo

```bash
cargo run -- --input examples/catalogo-sporco.csv \
             --output catalogo.json \
             --rejects scarti.jsonl \
             --default-currency EUR
```

Il CSV di esempio contiene dieci righe realisticamente sporche. Sulle sole
regole, senza alcun modello:

```
righe lette              10
ambigui dopo le regole    13
scartati                  13
```

Cosa ha risolto da solo:

| in ingresso | in uscita | |
|---|---|---|
| `ACME S.r.l.` e `acme srl` | `Acme` | due marche che nei filtri sarebbero rimaste distinte |
| `3m italia srl` | `3M` | grafia ufficiale, non ottenibile per regola |
| `nero opaco`, `matt black`, `SCHWARZ` | `nero` | tre lingue e una finitura |
| `120x60x2 cm` | `1200 × 600 × 20 mm` | unità decisa una volta sola, in ingresso |
| `47in` | `1193.8 mm` | |
| `1.234,56 €` | `123456` centesimi, `EUR` | interi, mai virgola mobile |
| `$129.99` | `12999` centesimi, `USD` | il simbolo batte la valuta predefinita |
| `400638133393` | `4006381333931` | 12 cifre completate con la cifra di controllo |

E cosa ha rifiutato, con la motivazione che finisce nel log:

```
SKU-004  ean    "4006381333930"        cifra di controllo EAN-13 non valida: il codice
                                       e' corrotto o trascritto male
SKU-006  color  "nero / bianco"        piu' colori nella stessa etichetta: serve una
                                       decisione, non una scelta automatica
SKU-006  price  "1,234 €"              separatore ambiguo: le tre cifre finali possono
                                       essere decimali o migliaia
SKU-003  ean    "96385074"             EAN-8: la conversione a EAN-13 richiede il
                                       prefisso aziendale, non deducibile dal codice
SKU-010  size   "1x2x3x4 cm"           attese da una a tre dimensioni separate da x
```

## Le decisioni che contano

**Non indovinare mai un numero ambiguo.** `1,234` vale 1234 in Italia e 1,234
negli Stati Uniti. Con due separatori il decimale è determinato e si legge; con
uno solo seguito da tre cifre, no — e la riga viene respinta. Sbagliare un prezzo
di tre ordini di grandezza su una parte del catalogo fa molto più danno di una
riga da rivedere a mano.

**Assente e ambiguo sono cose diverse.** Un campo vuoto all'origine non è un
errore e non va segnalato. Un campo pieno che le regole non sanno leggere è
esattamente ciò che merita attenzione. La distinzione è nel tipo:

```rust
pub enum Outcome<T> {
    Resolved(T),
    Absent,
    Ambiguous { raw: String, reason: String },
}
```

Il compilatore non lascia dimenticare nessuno dei tre casi, e la motivazione è
obbligatoria per costruzione: non si può marcare qualcosa come ambiguo senza dire
perché.

**I prezzi sono interi.** Centesimi in `i64`, mai virgola mobile. Su un catalogo
da centinaia di migliaia di righe gli errori di arrotondamento si accumulano, e un
prezzo è un dato contabile.

**Il costo si misura, non si stima.** Con `--llm` ogni chiamata contabilizza i
token effettivi restituiti dall'API e li converte in dollari con il listino del
modello scelto. Un modello fuori listino non fa inventare un costo: resta a zero,
dichiarato come tale.

## Il passaggio opzionale con il modello

```bash
export ANTHROPIC_API_KEY=...
cargo run -- --input catalogo.csv --llm --batch-size 50
```

Solo il residuo ambiguo viene inviato, in lotti, in una sola chiamata per lotto.
La richiesta usa gli **output strutturati** dell'API, così la risposta è JSON
valido per costruzione invece di doverla estrarre da testo libero. Lo sforzo è
impostato al minimo: è una mappatura meccanica, non un problema di ragionamento,
e lo sforzo basso taglia i token senza togliere nulla alla qualità.

Poi ogni valore proposto ripassa da `resolver::validate`, che riapplica la regola
del campo. Il report finale distingue le proposte accettate da quelle rifiutate,
così si vede subito se il modello sta lavorando o sta inventando:

```
risolti dal modello       9
proposte rifiutate        2
token                     1840 in / 210 out in 1 chiamate
costo                     $0.0145
```

## Struttura

```
src/
  model.rs        Outcome<T>, i tipi del catalogo (Dimensions, Money, Color)
  rules/
    numeric.rs    lettura dei numeri scritti da umani, e quando rifiutarsi
    dimensions.rs unità di misura → millimetri
    price.rs      importi → centesimi interi + valuta
    color.rs      sinonimi IT/EN/DE, finiture ignorate
    brand.rs      forme giuridiche rimosse, grafie ufficiali rispettate
    ean.rs        cifra di controllo EAN-13
  resolver.rs     il residuo: tratto, implementazione nulla, client API, validazione
  pipeline.rs     lettura → regole → residuo → validazione → scritture
  main.rs         CLI
```

60 test, tutti offline: l'implementazione predefinita del resolver non risolve
niente e non tocca la rete, quindi la suite gira senza chiave API.

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

---

È il mio primo progetto in Rust, scritto per entrare nello stack partendo da un
problema che conosco: vengo da due anni di gestione di cataloghi e-commerce e
sincronizzazione su marketplace, dove i dati arrivano esattamente così.
Suggerimenti su come renderlo più idiomatico sono benvenuti.
