# SplitSlip

Snap a warehouse-club receipt with names scribbled beside the items. SplitSlip reads the photo with **Gemini 2.5 Flash** (free tier), splits every item and its tax to the cent, and creates one **PayPal Invoicing API** invoice per person in the sandbox. It reminds whoever hasn't paid and rolls up what each person owes across runs.

Built for the [PayPal AI Hackathon](https://paypalaihackathon.devpost.com/).

## Why a generic splitter gets this wrong

A receipt is not a list of prices. It has:

- **Handwritten initials** beside lines: `L K` means that item is shared by two people.
- **Abbreviations**: `KS ORG EGGS 24CT` has to become something a person recognises.
- **Item-number coupons**: a `TPD/1002` line takes money off the earlier item numbered `1002`, wherever it sits.
- **Per-line tax flags**: only lines with a tax letter (`A`) are taxed, so tax has to be spread over those lines alone.
- **A printed total** that every cent must add up to.

SplitSlip uses the model only for the part code cannot do, reading the image into a small JSON schema. All money is integer cents, and the split is checked against the printed subtotal, tax and total. If a misread does not add up, the run stops and shows both numbers. Nothing is sent until you approve it.

Invoice lifecycle: `invoice` (send) → `remind` (nudge unpaid invoices after N days) → `mark-paid` (record a cash payment) → `rollup` (what each person owes and has paid).

## Try it with no keys

You need Rust (`cargo`). Install it with `brew install rust` or from [rustup.rs](https://rustup.rs).

```sh
cargo run -- scan fixtures/weekend.vision.json --people fixtures/people.json
```

This runs the full split on a synthetic receipt with no network calls and writes `out/weekend.json`. The last line reads `Sum of shares 48.15 = receipt total 48.15 OK`.

## Run it for real

1. Install Rust as above.
2. `cp .env.example .env` and fill it in:
   - `GEMINI_API_KEY`: a free key from [Google AI Studio](https://aistudio.google.com/apikey).
   - `PAYPAL_CLIENT_ID` and `PAYPAL_CLIENT_SECRET`: create a sandbox app at [developer.paypal.com](https://developer.paypal.com/dashboard) (Apps & Credentials, Sandbox tab). `PAYPAL_BASE` stays on the sandbox URL.
3. `cp people.example.json people.json` and put in the names and emails of the people on the receipt. **Use PayPal sandbox personal accounts for the emails.** Sandbox invoices are only delivered to sandbox accounts; with a real address, the invoice sends but nobody can pay it.
4. Run the commands in order:

```sh
mkdir -p out
cargo run -- vision receipts/trip.jpg > out/trip.vision.json   # optional: see exactly what the model read
cargo run -- scan receipts/trip.jpg                            # split it; writes out/trip.json
cargo run -- invoice out/trip.json                             # shows each share, asks y/N, sends the invoices
cargo run -- status out/trip.json                              # refresh statuses from PayPal
cargo run -- remind out/trip.json --days 3                     # nudge anyone unpaid after 3 days
cargo run -- mark-paid out/trip.json K                         # record a cash payment from K
cargo run -- rollup --month 2026-11                            # totals per person for the month
```

`rollup` reads the first 100 invoices on the account, which is plenty for a demo.

### Fixing a misread

If a line comes out wrong, save the model's reading, edit it, and scan the edited file. A `.json` input needs no key:

```sh
cargo run -- vision receipts/trip.jpg > out/trip.vision.json
$EDITOR out/trip.vision.json
cargo run -- scan out/trip.vision.json
```

## How the money works

- Each item's net is its price plus its coupon (coupons are negative and attach to the last earlier item with the same item number).
- Tax is spread over the taxable items in proportion to their net, with largest-remainder rounding so the cents add up exactly.
- An item's net plus tax is split equally among the people initialed on it, in cents. Leftover cents go to the first people listed. An item with no initials goes to `me` (the `me` field in `people.json`).
- The split must add up to the printed total, or the run stops with an error.

## Privacy

Real receipts, names and emails never go in the repo:

- `receipts/`, `out/`, `people.json` and `.env` are gitignored.
- `.env.example` and `people.example.json` hold placeholders only.
- Files in `fixtures/` use made-up names and amounts.

## Built with

- Rust with `serde`, `serde_json` and `ureq`. Base64 and `.env` loading are hand-rolled.
- Gemini 2.5 Flash (`generateContent`) for reading the receipt image.
- PayPal Invoicing API v2 (sandbox) for invoices, status, reminders and payments.

## License

MIT. See [LICENSE](LICENSE).
