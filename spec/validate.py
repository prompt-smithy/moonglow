#!/usr/bin/env python3
"""Validate the Moonglow behavioural spec.

Checks that the settings schema is a valid JSON Schema and that every golden
vector file parses, has the expected shape, and is internally consistent.

Run from anywhere: python3 spec/validate.py
"""

import json
import sys
from pathlib import Path

SPEC = Path(__file__).resolve().parent
VECTORS = SPEC / "vectors"
EPSILON = 1e-6

errors: list[str] = []


def fail(message: str) -> None:
    errors.append(message)


def load(path: Path):
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fail(f"{path.relative_to(SPEC.parent)}: invalid JSON: {exc}")
        return None


def check_schema() -> None:
    path = SPEC / "settings.schema.json"
    schema = load(path)
    if schema is None:
        return
    try:
        import jsonschema
    except ImportError:
        print("jsonschema not installed - skipping meta-schema check", file=sys.stderr)
        return
    try:
        jsonschema.Draft202012Validator.check_schema(schema)
    except jsonschema.exceptions.SchemaError as exc:
        fail(f"settings.schema.json: not a valid JSON Schema: {exc.message}")

    if schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
        fail("settings.schema.json: $schema must be the 2020-12 dialect")

    props = schema.get("properties", {})
    for key in ("dayColorTempK", "nightColorTempK", "morningStart", "morningEnd",
                "eveningStart", "eveningEnd"):
        if key not in props:
            fail(f"settings.schema.json: missing property {key!r}")


def check_kelvin() -> None:
    path = VECTORS / "kelvin.json"
    data = load(path)
    if data is None:
        return
    for key in ("description", "clamp", "vectors"):
        if key not in data:
            fail(f"kelvin.json: missing {key!r}")
            return
    lo, hi = data["clamp"]["minKelvin"], data["clamp"]["maxKelvin"]
    if not lo < hi:
        fail("kelvin.json: clamp minKelvin must be below maxKelvin")
    seen = set()
    for row in data["vectors"]:
        for key in ("kelvin", "red", "green", "blue"):
            if key not in row:
                fail(f"kelvin.json: row missing {key!r}: {row}")
                return
        k = row["kelvin"]
        if not lo <= k <= hi:
            fail(f"kelvin.json: kelvin {k} outside clamp [{lo}, {hi}]")
        if k in seen:
            fail(f"kelvin.json: duplicate kelvin {k}")
        seen.add(k)
        for channel in ("red", "green", "blue"):
            value = row[channel]
            if not 0.0 - EPSILON <= value <= 1.0 + EPSILON:
                fail(f"kelvin.json: {channel} {value} at {k} K is outside [0, 1]")
    if not seen:
        fail("kelvin.json: no vectors")


def check_schedule() -> None:
    path = VECTORS / "schedule.json"
    data = load(path)
    if data is None:
        return
    for key in ("description", "step", "settings", "cases"):
        if key not in data:
            fail(f"schedule.json: missing {key!r}")
            return
    step = data["step"]
    profiles = data["settings"]
    if "default" not in profiles:
        fail("schedule.json: settings must include a 'default' profile")
    for name, profile in profiles.items():
        night, day = profile["nightColorTempK"], profile["dayColorTempK"]
        if night > day:
            fail(f"schedule.json: profile {name!r} has night {night} above day {day}")
        for window in ("morningStart", "morningEnd", "eveningStart", "eveningEnd"):
            hour, minute = profile[window]
            if not (0 <= hour < 24 and 0 <= minute < 60):
                fail(f"schedule.json: profile {name!r} {window} is not a valid time")
    for case in data["cases"]:
        for key in ("settings", "time", "kelvin"):
            if key not in case:
                fail(f"schedule.json: case missing {key!r}: {case}")
                return
        if case["settings"] not in profiles:
            fail(f"schedule.json: case references unknown profile {case['settings']!r}")
        if len(case["time"]) != 5 or case["time"][2] != ":":
            fail(f"schedule.json: case time {case['time']!r} is not HH:mm")
        kelvin = case["kelvin"]
        if kelvin % step != 0:
            fail(f"schedule.json: case kelvin {kelvin} is not on the {step} K grid")


def check_temperature_range() -> None:
    path = VECTORS / "temperature-range.json"
    data = load(path)
    if data is None:
        return
    for key in ("description", "kelvinRange", "step", "snap", "range"):
        if key not in data:
            fail(f"temperature-range.json: missing {key!r}")
            return
    lo, hi = data["kelvinRange"]["min"], data["kelvinRange"]["max"]
    step = data["step"]
    for row in data["snap"]:
        output = row["output"]
        if not lo <= output <= hi:
            fail(f"temperature-range.json: snap output {output} outside [{lo}, {hi}]")
        if output % step != 0:
            fail(f"temperature-range.json: snap output {output} is not on the {step} K grid")
        # Inputs already inside the range may only move by half a step; out-of-range
        # inputs are expected to clamp all the way to an edge.
        if lo <= row["input"] <= hi and abs(row["output"] - row["input"]) > step / 2:
            fail(f"temperature-range.json: snap moved {row['input']} to {row['output']}, "
                 f"more than half a step")
    # The range vectors deliberately include rows whose inputs are swapped or out of
    # bounds; the contract is that the outputs always come back ordered and in range.
    for row in data["range"]:
        for key in ("night", "day", "nightOut", "dayOut"):
            if key not in row:
                fail(f"temperature-range.json: range row missing {key!r}: {row}")
                return
        for key in ("nightOut", "dayOut"):
            if not lo <= row[key] <= hi:
                fail(f"temperature-range.json: {key} {row[key]} outside [{lo}, {hi}]")
            if row[key] % step != 0:
                fail(f"temperature-range.json: {key} {row[key]} is not on the {step} K grid")
        if row["nightOut"] > row["dayOut"]:
            fail(f"temperature-range.json: outputs not ordered: "
                 f"{row['nightOut']} > {row['dayOut']}")


def main() -> int:
    check_schema()
    check_kelvin()
    check_schedule()
    check_temperature_range()

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        print(f"\n{len(errors)} problem(s) in the spec", file=sys.stderr)
        return 1

    print("spec OK: schema and golden vectors are consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
