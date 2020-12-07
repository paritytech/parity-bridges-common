In the scenarios, for simplicity, we call the chains Kusama (KSM token) and Polkadot (DOT token),
but they should be applicable to any other chains.

Notation:
- kX - user X interacting with Kusama chain.
- `k(kX)` - Kusama account id of user kX (native account id; usable on Kusama)
- `p(kX)` - Polkadot account id of user kX (account id derived from `k(kX)` usable on Polkadot)
- [Kusama] ... - Interaction happens on Kusama (i.e. the user interacts with Kusama chain)
- [Polkadot] ... - Interaction happens on Polkadot


Scenario 1: Kusama's Alice receiving & spending DOTs.
===========================

Kusama's Alice (kAlice) receives 5 DOTs from Polkadot's Bob (pBob) and sends half of them to
kCharlie.

1. Generate kAlice's DOT address (`p(kAlice)`)
2. [Polkadot] pBob transfers 5 DOTs to `p(kAlice)`
3. [Kusama] kAlice sends 2.5 DOTs to `p(kCharlie)`

UI challenges:
- The UI should warn before (or prevent) sending to `k(kCharlie)`!


Scenario 2: Kusama's Alice nominating validators with her DOTs.
===========================


Scenario 3: Kusama Treasury receing & spending DOTs.
===========================

Scenario 4: Kusama Treasury staking & nominating validators with DOTs.
===========================

Scenario 5: Kusama Treasury voting in Polkadot's democracy proposal.
===========================

More hipothetical scenarios?

Scenario 6: Polkadot's Bob spending his DOTs by using Kusama chain.
===========================
We can assume he holds KSM. Problem: he can pay fees, but can't really send (sign) a transaction?
Shall we support some kind of dispatcher?

Scenario 7: Kusama Governance taking over Kusama's Alice DOT holdings.
===========================
We use `SourceRoot` call to transfer her's DOTs to Kusama treasury.
