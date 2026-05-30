#!/usr/bin/env python3
from __future__ import annotations

import json

RECORDS = 3_000_000
SEED0 = 20_260_216
MOD = 2_147_483_647


def next_seed(seed: int) -> int:
    x = seed * 1_103_515_245 + 12_345
    return x % MOD


def score_transaction(amount: int, velocity: int, account_age_days: int, country_risk: int) -> int:
    score = 0

    if amount > 12_000:
        score += 25
    if amount > 26_000:
        score += 20

    if velocity > 6:
        score += velocity * 2
    if velocity > 12:
        score += 10

    if account_age_days < 60:
        score += 20

    if country_risk > 70:
        score += 18
    if country_risk > 90:
        score += 12

    return score


def run() -> dict[str, int]:
    seed = SEED0
    score_sum = 0
    flagged = 0

    for _ in range(RECORDS):
        seed = next_seed(seed)
        amount = seed % 50_000
        velocity = (seed // 17) % 20
        account_age_days = (seed // 29) % 3_650
        country_risk = (seed // 37) % 100

        score = score_transaction(amount, velocity, account_age_days, country_risk)
        score_sum += score
        if score > 75:
            flagged += 1

    return {"records": RECORDS, "score_sum": score_sum, "flagged": flagged}


def main() -> int:
    print(json.dumps(run(), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
