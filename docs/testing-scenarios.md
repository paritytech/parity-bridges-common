In the scenarios, for simplicity, we call the chains Kusama (KSM token) and Polkadot (DOT token),
but they should be applicable to any other chains.

Notation:
- kX - user X interacting with Kusama chain.
- `k(kX)` - Kusama account id of user kX (native account id; usable on Kusama)
- `p(kX)` - Polkadot account id of user kX (account id derived from `k(kX)` usable on Polkadot)
- [Kusama] ... - Interaction happens on Kusama (i.e. the user interacts with Kusama chain)
- [Polkadot] ... - Interaction happens on Polkadot

Basic Scenarios
===========================

Scenario 1: Kusama's Alice receiving & spending DOTs.
---------------------------

Kusama's Alice (kAlice) receives 5 DOTs from Polkadot's Bob (pBob) and sends half of them to
kCharlie.

1. Generate kAlice's DOT address (`p(kAlice)`)
2. [Polkadot] pBob transfers 5 DOTs to `p(kAlice)`
3. [Kusama] kAlice sends 2.5 DOTs to `p(kCharlie)`

UI challenges:
- The UI should warn before (or prevent) sending to `k(kCharlie)`!


Scenario 2: Kusama's Alice nominating validators with her DOTs.
---------------------------

kAlice receives 10 DOTs from pBob and nominates `p(pCharlie)` and `p(pDave)`.

1. Generate kAlice's DOT address (`p(kAlice)`)
2. [Polkadot] pBob transfers 5 DOTs to `p(kAlice)`
3. [Kusama] kAlice sends a batch transaction:
  - `staking::Bond` transaction to create stash account choosing `p(kAlice)` as the controller account.
  - `staking::Nominate(vec![p(pCharlie)])` to nominate pCharlie using the controller account.


Scenario 3: Kusama Treasury receiving & spending DOTs.
---------------------------

pBob sends 15 DOTs to Kusama Treasury which Kusama Governance decides to transfer to kCharlie.

1. Generate source account for the treasury (`kTreasury`).
2. [Polkadot] pBob tarnsfers 15 DOTs to `p(kTreasury)`.
2. [Kusama] Send a governance proposal to send a bridge message which transfers funds to `p(kCharlie)`.
3. [Kusama] Dispatch the governance proposal using `kTreasury` account id.

Extra scenarios
===========================

Scenario 4: Kusama's Alice setting up 1-of-2 multi-sig to spend from either Kusama or Polkadot
---------------------------

Assuming `p(pAlice)` has at least 7 DOTs already.

1. Generate multisig account id: `pMultiSig = multi_account_id(&[p(kAlice), p(pAlice)], 1)`.
2. [Kusama] Transfer 7 DOTs to `pMultiSig` using `TargetAccount` origin of `pAlice`.
3. [Kusama] Transfer 2 DOTs to `p(kAlice)` from the multisig:
   - Send `multisig::as_multi_threshold_1(vec![p(pAlice)], balances::Transfer(p(kAlice), 2))`

Scenario 5: Kusama Treasury staking & nominating validators with DOTs.
---------------------------

Scenario 6: Kusama Treasury voting in Polkadot's democracy proposal.
---------------------------

Potentially interesting scenarios
===========================

Scenario 7: Polkadot's Bob spending his DOTs by using Kusama chain.
---------------------------

We can assume he holds KSM. Problem: he can pay fees, but can't really send (sign) a transaction?
Shall we support some kind of dispatcher?

Scenario 8: Kusama Governance taking over Kusama's Alice DOT holdings.
---------------------------

We use `SourceRoot` call to transfer her's DOTs to Kusama treasury.
