# SplitSlip

Snap a Costco receipt with names scribbled on it. An AI agent decodes the cryptic item lines and coupons, splits items and tax, sends each friend a PayPal invoice, and nudges whoever hasn't paid.

Built for the [PayPal AI Hackathon](https://paypalaihackathon.devpost.com/) (Oct 1 – Nov 12, 2026).

## Status

Planning. See the issues for the plan and progress.

## How it works (planned)

1. Photo of the receipt, with a name or initial written next to each item.
2. A vision model reads the lines, expands names like `KS ORG EGGS 24CT`, and reads the handwriting.
3. Code matches coupon lines (`TPD/1234567 3.00-`) to their items and applies taxable flags.
4. You approve the split.
5. Each person gets a PayPal invoice (sandbox), with reminders until it's paid.

## Running it

Setup instructions will land here with the first working build. Everything runs against the free PayPal sandbox.

## License

MIT
