#!/usr/bin/env python3
"""Migration front — a console over Hyperswitch's routing-rule migration endpoints.

Serves `index.html` and proxies to Hyperswitch, holding the admin API key in this process so
it never reaches the browser:

    GET  /api/state                 last scan, its totals, and any running job
    POST /api/config                point at an environment: urls, credentials, extra headers
    POST /api/environment           switch to another environment already configured here
    POST /api/scan                  page through GET  /routing/migration/status
    POST /api/migrate               batch  through POST /routing/rule/migrate
    POST /api/cutover               switch routing over with PUT /context on Superposition
    POST /api/de-scopes             provision missing scopes through POST /routing/rule/migrate

The environment is set from the dashboard rather than the command line, because the key for a
hosted environment is pasted in when it is needed and the extra headers differ per environment
(sandbox needs `x-feature`). Sandbox and each production region are held side by side rather
than one at a time, so moving between them is a pick from the header rather than three URLs and
three credentials typed again. Nothing is written to disk unless "remember" is asked for.

An environment says what it is, and a production one is marked as such wherever it is shown and
takes its name typed back before a migration, a cutover or a scope run — every write against it
lands on live merchants.

`/routing/migration/status` reports one page at a time and each profile in a page costs the
router one call to the decision engine, so a scan is a background job that pages with progress,
and a scan of a large estate is resumed page by page rather than run to the end in one go.

That status is about rules only. Whether the decision engine holds a *scope* for a profile is a
question it alone can answer, so a scan also asks it — see `Dashboard.check_scopes`.

Only the standard library is used: this is an operations tool that has to run from a checkout
during a migration window, with no environment to prepare first.

    ./server.py                     then set the environment in the browser

"""

from __future__ import annotations

import argparse
import copy
import enum
import json
import os
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
CONFIG_FILE = (
    Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    / "hs-migration-dashboard"
    / "config.json"
)

# What the dashboard opens on. Hyperswitch's hosted sandbox routes a request to a release train
# by `x-feature`, so the header travels with every call rather than only the first.
DEFAULT_HS_URL = "https://sandbox.hyperswitch.io"
DEFAULT_HEADERS = {"x-feature": "sandbox-custom-c2"}

# The saved file holds every environment; older files held the one that was in use.
CONFIG_VERSION = 2


class EnvKind(enum.Enum):
    """What an environment is, which is what decides how much ceremony a write to it takes.

    A name this version does not know reads as OTHER instead of failing, so a file written by a
    later one — or edited by hand — still opens."""

    SANDBOX = "sandbox"
    PRODUCTION = "production"
    LOCAL = "local"
    OTHER = "other"

    @classmethod
    def _missing_(cls, value):
        return cls.OTHER

    @property
    def live(self) -> bool:
        """Whether a write here reaches real merchants."""
        return self is EnvKind.PRODUCTION


# The environments this is pointed at in practice, always offered in the picker even before one
# has been configured. They carry addresses and nothing else: a credential belongs to whoever is
# running the window, and is pasted in when it is needed.
ENV_PRESETS = [
    {
        "name": "sandbox",
        "label": "Sandbox",
        "kind": EnvKind.SANDBOX.value,
        "hyperswitch_url": DEFAULT_HS_URL,
        "headers": dict(DEFAULT_HEADERS),
    },
    {
        "name": "production-eu",
        "label": "Production EU",
        "kind": EnvKind.PRODUCTION.value,
        # The production regions serve the API under `/api` on the region host, so the base URL
        # carries that path and every endpoint below hangs off it.
        "hyperswitch_url": "https://eu.hyperswitch.io/api",
        "headers": {},
    },
]

# Cutting a profile over is a Superposition override, not a Hyperswitch call: the router reads
# `routing.routing_result_source` per profile and asks whoever it names for the routing result.
# The key is editable because it is the one thing that fails quietly — an override written under
# a key the router does not read is accepted and changes nothing, which is why every cutover run
# re-reads the profiles afterwards and reports what the router now says.
CUTOVER_KEY = "routing.routing_result_source"
# Superposition refuses an override whose key its workspace has no schema for, and this config
# was moved into a `routing.` folder at one point, so an environment answers to whichever name it
# was seeded with. That refusal is proof the name is not defined there, which is what makes
# retrying under the other one safe rather than a guess.
SCHEMA_REFUSAL = "failed to get schema for config key"
DEFAULT_SUPERPOSITION_HEADERS = {"x-tenant": "hyperswitch", "x-org-id": "hyperswitch"}
SOURCE_DECISION_ENGINE = "decision_engine"
SOURCE_HYPERSWITCH = "hyperswitch_routing"
# Which states each direction may be applied to. Cutting over a profile whose rules have not all
# crossed would route live traffic against a decision engine that is missing them.
CUTOVER_FROM = {"migrated"}
ROLLBACK_FROM = {"enabled", "enabled_without_rules"}

# The decision engine's read-only reconcile, gated by the shared admin secret rather than an API
# key. It reports which profiles it has no scope for, and is the only question Hyperswitch cannot
# answer — which is why the decision engine needs its own address here.
#
# Its sibling `/admin/hierarchy/sync` is deliberately not called from here. Creating a scope means
# writing the organization and merchant above the profile, and this tool would have to name them
# from the status feed — which carries the provider rather than the owner for a platform-written
# rule, and carries no profile or organization name at all. Hyperswitch has all of that in hand
# and syncs the hierarchy itself; see `run_scope_provision`.
DE_RECONCILE_PATH = "/admin/hierarchy/reconcile"

# Hyperswitch stamps every response with this and writes its own logs under it. It is the only
# handle on a failure whose body explains nothing — `HE_00 Something went wrong` is the router
# declining to say what happened, and what happened is in its log under this id — so it is kept
# with the outcome it belongs to rather than read once and dropped.
REQUEST_ID_HEADER = "x-request-id"

# Hyperswitch caps the status page at 500 and the migration batch at 1000 profiles; both
# defaults here sit under the cap so a slow decision engine cannot stall a whole run.
STATUS_PAGE_SIZE = 500
MIGRATE_BATCH_SIZE = 25

# Pages read per scan; 0 is every page there is. An estate is thousands of profiles, not
# millions — a few pages of 500 — so a scan reads the whole feed and the table is complete the
# first time it is looked at, rather than growing under whoever is working through it.
SCAN_PAGES_DEFAULT = 0

# `/routing/rule/migrate` reads this many rules *per profile in the batch* and defaults to 50,
# so a profile with more rules than this would quietly migrate only its first page. The request
# asks for the cap, and any profile holding more than a page is walked with offsets below.
RULE_PAGE_LIMIT = 1000

# States where Hyperswitch holds rules the decision engine does not, i.e. what a migration run
# would write. `unknown` is excluded: the decision engine could not be read, so a migration
# would be firing blind.
UNFINISHED_STATES = {"pending", "partial", "diverged", "enabled_without_rules"}


class Target:
    """The environment being worked on, changeable while the server runs."""

    def __init__(self, hs_url: str, api_key: str = "", headers=None, label: str = "",
                 name: str = "", kind=EnvKind.OTHER):
        self.lock = threading.Lock()
        self.set(hs_url, api_key, headers or {}, label, name, kind)

    def set(self, hs_url: str, api_key: str, headers: dict, label: str,
            name: str = "", kind=EnvKind.OTHER):
        with self.lock:
            self.hs_url = (hs_url or "").rstrip("/")
            self.api_key = api_key or ""
            self.headers = clean_headers(headers)
            self.label = label or host_of(self.hs_url)
            # The label is what an environment is called on screen; the name is that label as a
            # key — what the saved file files it under and what a production write is confirmed
            # with, so it stays stable while the label is being typed.
            self.name = (name or "").strip() or slug_of(self.label)
            self.kind = EnvKind(kind)

    def snapshot(self):
        with self.lock:
            return self.hs_url, self.api_key, dict(self.headers)

    def identity(self):
        with self.lock:
            return self.name, self.label, self.kind

    def public(self):
        """What the browser is told: everything except the key itself."""
        with self.lock:
            return {
                "hyperswitch_url": self.hs_url,
                "label": self.label,
                "name": self.name,
                "kind": self.kind.value,
                "live": self.kind.live,
                "headers": [{"name": k, "value": v} for k, v in sorted(self.headers.items())],
                "api_key_set": bool(self.api_key),
                "api_key_hint": ("…" + self.api_key[-4:]) if len(self.api_key) > 4 else "",
            }


class Superposition:
    """Where cutovers are written. Configured from the dashboard like the environment itself,
    because the secret belongs to whoever is running the migration window."""

    def __init__(self, url: str = "", secret: str = "", headers=None, config_key: str = CUTOVER_KEY):
        self.lock = threading.Lock()
        self.set(url, secret, headers or dict(DEFAULT_SUPERPOSITION_HEADERS), config_key)

    def set(self, url: str, secret: str, headers: dict, config_key: str):
        with self.lock:
            self.url = (url or "").rstrip("/")
            if self.url.endswith("/context"):
                self.url = self.url[: -len("/context")]
            self.secret = secret or ""
            self.headers = clean_headers(headers)
            self.config_key = (config_key or "").strip() or CUTOVER_KEY

    def snapshot(self):
        with self.lock:
            return self.url, self.secret, dict(self.headers), self.config_key

    def adopt_key(self, key: str):
        """Keep the name this workspace actually answered to, so the rest of the run and every
        run after it goes straight there."""
        with self.lock:
            self.config_key = key

    def public(self):
        with self.lock:
            return {
                "url": self.url,
                "headers": [{"name": k, "value": v} for k, v in sorted(self.headers.items())],
                "config_key": self.config_key,
                "secret_set": bool(self.secret),
                "ready": bool(self.url and self.secret),
            }


class DecisionEngine:
    """The decision engine itself, which the rest of this tool only ever reaches through
    Hyperswitch. It is addressed directly for one question Hyperswitch cannot answer: whether a
    profile exists there as a scope at all. Optional — leave it unset and the scope column
    simply says it was not read."""

    def __init__(self, url: str = "", admin_secret: str = "", headers=None):
        self.lock = threading.Lock()
        self.set(url, admin_secret, headers or {})

    def set(self, url: str, admin_secret: str, headers: dict):
        with self.lock:
            self.url = (url or "").rstrip("/")
            # The address is usually copied from a curl of one of the hierarchy endpoints, so a
            # path on the end of it is dropped rather than turned into `…/reconcile/admin/…`.
            for path in (DE_RECONCILE_PATH, "/admin/hierarchy/sync"):
                if self.url.endswith(path):
                    self.url = self.url[: -len(path)]
            self.admin_secret = admin_secret or ""
            self.headers = clean_headers(headers)

    def snapshot(self):
        with self.lock:
            return self.url, self.admin_secret, dict(self.headers)

    def public(self):
        with self.lock:
            return {
                "url": self.url,
                "headers": [{"name": k, "value": v} for k, v in sorted(self.headers.items())],
                "secret_set": bool(self.admin_secret),
                "ready": bool(self.url and self.admin_secret),
            }


class Environments:
    """Every environment this console knows about, of which exactly one is in use.

    The one in use is the Target, Superposition and DecisionEngine above; the rest sit here as
    plain records. Switching writes the three back into the record they came from and loads the
    next one over them, so going from sandbox to production and back is a pick rather than six
    addresses and three credentials typed again.

    Credentials are held here for as long as the process runs whether or not they were
    remembered on disk: a key pasted in for a production window should survive a look at sandbox
    in the middle of it."""

    def __init__(self, records=None):
        self.lock = threading.Lock()
        self.records: dict[str, dict] = {}
        for record in records or []:
            record = normalise_env(record)
            self.records[record["name"]] = record

    def seed(self, presets):
        """Offer the known environments even before one has been configured. A preset is skipped
        where the store already holds that name or that address — the saved record is the one
        with the credentials, and a second row for the same host under another name would be a
        second place to look for them."""
        with self.lock:
            held = {record["hyperswitch_url"] for record in self.records.values()}
            for preset in presets:
                record = normalise_env(preset)
                if record["name"] in self.records or record["hyperswitch_url"] in held:
                    continue
                self.records[record["name"]] = record

    def remember(self, record: dict):
        with self.lock:
            self.records[record["name"]] = normalise_env(record)

    def rename(self, old: str, record: dict):
        """An environment renamed in the panel moves rather than forks, or the picker would
        offer both the old name and the new one with the credentials only under one of them."""
        with self.lock:
            record = normalise_env(record)
            if old != record["name"]:
                self.records.pop(old, None)
            self.records[record["name"]] = record

    def get(self, name: str):
        with self.lock:
            record = self.records.get(name)
            return copy.deepcopy(record) if record else None

    def records_list(self):
        with self.lock:
            return [copy.deepcopy(record) for record in self.records.values()]

    def public(self, active: str):
        """The picker: what each environment is called, where it points, and whether its
        credentials are already in hand — never the credentials themselves."""
        return [
            {
                "name": record["name"],
                "label": record["label"],
                "kind": record["kind"],
                "live": EnvKind(record["kind"]).live,
                "hyperswitch_url": record["hyperswitch_url"],
                "api_key_set": bool(record.get("api_key")),
                "current": record["name"] == active,
            }
            for record in self.records_list()
        ]


def slug_of(text: str) -> str:
    """A label as a key: what the saved file files an environment under, and what a production
    write is confirmed with."""
    kept = "".join(c if c.isalnum() else "-" for c in (text or "").lower())
    return "-".join(part for part in kept.split("-") if part) or "environment"


def guess_kind(url: str) -> EnvKind:
    """What an address looks like, used as the answer offered for a URL that was just typed and
    for a config saved before environments said what they were. It is a first answer and not a
    finding: an environment carries its own kind, and a production deployment on a domain this
    does not recognise has to be marked as one in the panel."""
    host = host_of(url).lower()
    if not host or host.split(":")[0] in ("localhost", "127.0.0.1", "[::1]"):
        return EnvKind.LOCAL
    if "sandbox" in host or "integ" in host:
        return EnvKind.SANDBOX
    if host.endswith("hyperswitch.io"):
        return EnvKind.PRODUCTION
    return EnvKind.OTHER


def normalise_env(record: dict) -> dict:
    """One environment's record, with everything the rest of this file expects it to have."""
    record = copy.deepcopy(dict(record or {}))
    url = (record.get("hyperswitch_url") or "").rstrip("/")
    sp = dict(record.get("superposition") or {})
    de = dict(record.get("decision_engine") or {})
    record.update(
        {
            "hyperswitch_url": url,
            "label": (record.get("label") or "").strip() or host_of(url),
            "headers": clean_headers(record.get("headers") or {}),
            "superposition": {
                "url": (sp.get("url") or "").strip(),
                "secret": sp.get("secret") or "",
                "headers": clean_headers(sp.get("headers") or dict(DEFAULT_SUPERPOSITION_HEADERS)),
                "config_key": (sp.get("config_key") or "").strip() or CUTOVER_KEY,
            },
            "decision_engine": {
                "url": (de.get("url") or "").strip(),
                "admin_secret": de.get("admin_secret") or "",
                "headers": clean_headers(de.get("headers") or {}),
            },
        }
    )
    record["name"] = (record.get("name") or "").strip() or slug_of(record["label"])
    record["kind"] = EnvKind(record.get("kind") or guess_kind(url)).value
    record["api_key"] = record.get("api_key") or ""
    return record


def env_record(target: Target, superposition, decision_engine) -> dict:
    """The environment on screen, as a record the store can hold."""
    hs_url, api_key, headers = target.snapshot()
    name, label, kind = target.identity()
    sp_url, sp_secret, sp_headers, config_key = superposition.snapshot()
    de_url, de_secret, de_headers = decision_engine.snapshot()
    return normalise_env(
        {
            "name": name,
            "label": label,
            "kind": kind.value,
            "hyperswitch_url": hs_url,
            "api_key": api_key,
            "headers": headers,
            "superposition": {
                "url": sp_url,
                "secret": sp_secret,
                "headers": sp_headers,
                "config_key": config_key,
            },
            "decision_engine": {
                "url": de_url,
                "admin_secret": de_secret,
                "headers": de_headers,
            },
        }
    )


def apply_env(record: dict, target: Target, superposition, decision_engine):
    """Point the three services at one environment's record."""
    record = normalise_env(record)
    target.set(
        record["hyperswitch_url"],
        record["api_key"],
        record["headers"],
        record["label"],
        record["name"],
        record["kind"],
    )
    sp = record["superposition"]
    superposition.set(sp["url"], sp["secret"], sp["headers"], sp["config_key"])
    de = record["decision_engine"]
    decision_engine.set(de["url"], de["admin_secret"], de["headers"])


def without_secrets(record: dict) -> dict:
    record = copy.deepcopy(record)
    record["api_key"] = ""
    record["superposition"]["secret"] = ""
    record["decision_engine"]["admin_secret"] = ""
    return record


def clean_headers(headers: dict) -> dict:
    """Header values reach a request line, so anything that could open a second one is dropped
    rather than escaped."""
    return {
        str(name).strip(): str(value).strip()
        for name, value in (headers or {}).items()
        if str(name).strip() and "\r" not in f"{name}{value}" and "\n" not in f"{name}{value}"
    }


def alternate_key(key: str) -> str:
    """The same config under its other name: foldered if it is flat, flat if it is foldered."""
    return key.rsplit(".", 1)[-1] if "." in key else "routing." + key


def host_of(url: str) -> str:
    return urllib.parse.urlparse(url).netloc or url


def load_saved():
    """The saved file as this version reads it: a list of environments and which one was last in
    use. A file from before environments were named holds one environment at the top level, and
    is read as that."""
    try:
        saved = json.loads(CONFIG_FILE.read_text())
    except (OSError, ValueError):
        return None
    if not isinstance(saved, dict):
        return None
    if isinstance(saved.get("environments"), list):
        return {
            "active": saved.get("active") or "",
            "environments": [normalise_env(record) for record in saved["environments"]],
        }
    if not saved.get("hyperswitch_url"):
        return None
    # That one environment was never named, so a known address names it: the file is from before
    # the picker existed, and it should come back as the entry the picker offers.
    record = normalise_env(saved)
    preset = next(
        (p for p in ENV_PRESETS if p["hyperswitch_url"] == record["hyperswitch_url"]), None
    )
    if preset:
        record.update({"name": preset["name"], "label": preset["label"], "kind": preset["kind"]})
    return {"active": record["name"], "environments": [record]}


def save_config(environments: Environments, active: str, keep_secrets: bool):
    """Every environment is written, not only the one on screen — the point of naming them is
    that a switch does not mean typing an address in again.

    `keep_secrets` is the "credentials are wanted on disk" answer, and it covers all of them at
    once: they are remembered together or not at all, since none is useful without the others."""
    CONFIG_FILE.parent.mkdir(parents=True, exist_ok=True)
    records = environments.records_list()
    payload = {
        "version": CONFIG_VERSION,
        "active": active,
        "environments": records if keep_secrets else [without_secrets(r) for r in records],
    }
    # Written before the mode is set, so the file is never briefly world-readable with a key in
    # it: opened by hand at 0600 rather than through write_text.
    fd = os.open(CONFIG_FILE, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as handle:
        json.dump(payload, handle, indent=2)


def forget_config():
    try:
        CONFIG_FILE.unlink()
    except FileNotFoundError:
        pass


def describe_failure(err, service: str, method: str, url: str) -> str:
    """Which call failed, and then what it answered.

    Three services answer into the same error line and any of them can say 401 — a body on its
    own does not name whose credential was refused, or which environment's. The name and the
    endpoint come first so they survive the truncation below."""
    body = err.read().decode("utf-8", "replace").strip()
    where = f"{service} {method} {url}"
    request_id = request_id_of(err.headers)
    if request_id:
        where += f" [{REQUEST_ID_HEADER} {request_id}]"
    return f"{where} — {body}" if body else f"{where} — no response body"


def request_id_of(headers) -> str:
    return (headers.get(REQUEST_ID_HEADER) or "").strip() if headers else ""


class UpstreamError(Exception):
    """A call to Hyperswitch, Superposition or the decision engine that came back non-2xx,
    carrying its status and body."""

    def __init__(self, status: int, body: str, request_id: str = ""):
        super().__init__(f"HTTP {status}: {body[:400]}")
        self.status = status
        self.body = body
        self.request_id = request_id


def hs_call(target: Target, method: str, path: str, query=None, body=None, timeout: int = 300,
            with_request_id: bool = False):
    """`with_request_id` returns `(payload, x-request-id)` instead of the payload alone, for a
    call whose *answer* can carry a failure — a batch that comes back 200 with a profile the
    router could not migrate is only traceable through the id of the request that carried it."""
    hs_url, api_key, headers = target.snapshot()
    _, label, _ = target.identity()
    # The environment is part of the name: with several configured, "Hyperswitch refused the
    # key" is only half an answer.
    service = f"Hyperswitch {label}"
    if not hs_url:
        raise UpstreamError(0, "no environment set — open Environment and give a URL")
    if not api_key:
        raise UpstreamError(
            0, f"{label} has no API key set — open Environment and paste the admin API key"
        )
    url = hs_url + path
    if query:
        url += "?" + urllib.parse.urlencode(query)
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(url, data=data, method=method)
    request.add_header("api-key", api_key)
    request.add_header("Accept", "application/json")
    for name, value in headers.items():
        request.add_header(name, value)
    if data is not None:
        request.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
            request_id = request_id_of(response.headers)
    except urllib.error.HTTPError as err:
        raise UpstreamError(
            err.code,
            describe_failure(err, service, method, hs_url + path),
            request_id_of(err.headers),
        ) from None
    except urllib.error.URLError as err:
        raise UpstreamError(0, f"{service} at {hs_url} unreachable: {err.reason}") from None
    payload = json.loads(raw) if raw else None
    return (payload, request_id) if with_request_id else payload


def superposition_call(sp: Superposition, method: str, path: str, body=None, timeout: int = 60):
    url, secret, headers, _ = sp.snapshot()
    if not url:
        raise UpstreamError(0, "no Superposition URL set — open Environment and give one")
    if not secret:
        raise UpstreamError(0, "no Superposition secret set — open Environment and paste it")
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(url + path, data=data, method=method)
    request.add_header("x-superposition-secret", secret)
    request.add_header("Accept", "application/json")
    for name, value in headers.items():
        request.add_header(name, value)
    if data is not None:
        request.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
    except urllib.error.HTTPError as err:
        raise UpstreamError(err.code, describe_failure(err, "Superposition", method, url + path)) from None
    except urllib.error.URLError as err:
        raise UpstreamError(0, f"Superposition at {url} unreachable: {err.reason}") from None
    return json.loads(raw) if raw else None


def de_call(de: DecisionEngine, method: str, path: str, body=None, timeout: int = 120):
    url, admin_secret, headers = de.snapshot()
    if not url:
        raise UpstreamError(0, "no decision engine URL set — open Environment and give one")
    if not admin_secret:
        raise UpstreamError(0, "no decision engine admin secret set — open Environment and paste it")
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(url + path, data=data, method=method)
    request.add_header("x-admin-secret", admin_secret)
    request.add_header("Accept", "application/json")
    for name, value in headers.items():
        request.add_header(name, value)
    if data is not None:
        request.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
    except urllib.error.HTTPError as err:
        raise UpstreamError(err.code, describe_failure(err, "decision engine", method, url + path)) from None
    except urllib.error.URLError as err:
        raise UpstreamError(0, f"decision engine at {url} unreachable: {err.reason}") from None
    return json.loads(raw) if raw else None


def merge_profile_result(by_profile, order, profile, request_id: str = ""):
    """Fold one profile's result into the run. A profile appears once per page of its rules, so
    a large profile's outcome is the union of its passes rather than whichever page came last.

    The batch's request id is kept with it, and a profile walked in several passes keeps one per
    pass: an outcome that says only "something went wrong" is looked up in Hyperswitch's own log
    by the id of the request that produced it, so the two are reported together."""
    profile_id = profile["profile_id"]
    existing = by_profile.get(profile_id)
    if existing is None:
        profile["request_ids"] = [request_id] if request_id else []
        by_profile[profile_id] = profile
        order.append(profile_id)
        return
    for key in ("success", "skipped", "errors", "not_applicable"):
        existing.setdefault(key, []).extend(profile.get(key) or [])
    if request_id and request_id not in existing.setdefault("request_ids", []):
        existing["request_ids"].append(request_id)
    if profile.get("not_attempted") and not existing.get("not_attempted"):
        existing["not_attempted"] = profile["not_attempted"]


class Job:
    """One background run — a scan or a migration — polled by the browser through /api/state."""

    def __init__(self, kind: str):
        self.kind = kind
        self.lock = threading.Lock()
        self.thread: threading.Thread | None = None
        self.reset()

    def reset(self):
        self.running = False
        self.done = 0
        self.total = 0
        self.message = ""
        self.error: str | None = None
        self.started_at: float | None = None
        self.finished_at: float | None = None
        self.result = None

    def snapshot(self):
        with self.lock:
            return {
                "kind": self.kind,
                "running": self.running,
                "done": self.done,
                "total": self.total,
                "message": self.message,
                "error": self.error,
                "started_at": self.started_at,
                "finished_at": self.finished_at,
                "result": self.result,
            }

    def progress(self, done=None, total=None, message=None):
        with self.lock:
            if done is not None:
                self.done = done
            if total is not None:
                self.total = total
            if message is not None:
                self.message = message

    def start(self, target, *args):
        with self.lock:
            if self.running:
                return False
            self.reset()
            self.running = True
            self.started_at = time.time()

        def run():
            try:
                result = target(*args)
                with self.lock:
                    self.result = result
            except Exception as err:  # surfaced in the UI rather than only in this terminal
                with self.lock:
                    self.error = str(err)
            finally:
                with self.lock:
                    self.running = False
                    self.finished_at = time.time()

        self.thread = threading.Thread(target=run, daemon=True)
        self.thread.start()
        return True


class Dashboard:
    def __init__(self, target: Target, superposition: Superposition,
                 decision_engine: DecisionEngine, environments: Environments):
        self.target = target
        self.superposition = superposition
        self.decision_engine = decision_engine
        self.environments = environments
        self.scan_job = Job("scan")
        self.migrate_job = Job("migrate")
        self.cutover_job = Job("cutover")
        self.scope_job = Job("scopes")
        self.lock = threading.Lock()
        self.scan_result = None
        self.rows: list[dict] = []       # every profile read so far, in the order read
        self.next_offset = 0             # where the next page of the status feed resumes
        self.has_more = True
        # merchant_id -> {"organization_id", "merchant_name"}, successful lookups only. A failed
        # one is not cached: it can mean the account is genuinely gone, or that Hyperswitch was
        # briefly unable to read it, and caching the second case would strand those profiles as
        # unmigratable for the life of this process.
        self.merchants: dict[str, dict] = {}

    def forget_scan(self):
        """Drop everything read from the previous environment. Profile and merchant ids are not
        comparable across environments, so carrying a scan over would show one estate's rows
        under another one's name."""
        with self.lock:
            self.scan_result = None
            self.rows = []
            self.next_offset = 0
            self.has_more = True
            self.merchants = {}
        self.migrate_job.reset()
        self.cutover_job.reset()
        self.scope_job.reset()
        self.scan_job.reset()

    # ------------------------------------------------------------ environments

    def remember_current(self, previous_name: str = ""):
        """Park what is on screen back in the store, under the name it now goes by."""
        record = env_record(self.target, self.superposition, self.decision_engine)
        self.environments.rename(previous_name or record["name"], record)
        return record

    def use_environment(self, record: dict):
        """Load another environment over the one on screen. The scan goes with it: profile ids
        are not comparable across environments, so carrying it would show one estate's rows
        under another one's name."""
        self.remember_current()
        apply_env(record, self.target, self.superposition, self.decision_engine)
        self.forget_scan()

    # ---------------------------------------------------------------- scanning

    def run_scan(self, pages: int, restart: bool):
        """Read `pages` more pages of the status feed, or every remaining page when pages is 0.

        A restart re-reads from the first page; anything else continues from where the last scan
        stopped, so a large estate is walked in pieces without losing what is already on screen.
        """
        job = self.scan_job
        with self.lock:
            if restart:
                self.rows = []
                self.next_offset = 0
                self.has_more = True
            rows = list(self.rows)
            offset = self.next_offset
            seen = {row["profile_id"] for row in rows}

        read = 0
        # Held locally and published with the rows it belongs to. Reported live, it would say
        # the whole estate is read while the rows from that last page are still being resolved.
        has_more = self.has_more
        job.progress(done=len(rows), message="reading migration status")
        while has_more and (pages == 0 or read < pages):
            page = hs_call(
                self.target,
                "GET",
                "/routing/migration/status",
                query={"limit": STATUS_PAGE_SIZE, "offset": offset},
            )
            profiles = page.get("profiles", [])
            for profile in profiles:
                # A profile added upstream between two pages shifts the window, which would
                # otherwise show the same profile twice.
                if profile["profile_id"] not in seen:
                    seen.add(profile["profile_id"])
                    rows.append(profile)
            offset += STATUS_PAGE_SIZE
            read += 1
            has_more = bool(page.get("has_more"))
            job.progress(
                done=len(rows),
                total=len(rows) + (STATUS_PAGE_SIZE if has_more else 0),
                message=f"{len(rows)} profiles read",
            )

        self.enrich_merchants(rows, job)
        scopes = self.check_scopes(rows, job)
        for row in rows:
            self.decorate(row)

        result = {
            "scanned_at": time.time(),
            "rows": rows,
            "totals": self.totals(rows),
            "scopes": scopes,
            "has_more": has_more,
            "pages_read": (offset // STATUS_PAGE_SIZE),
        }
        with self.lock:
            self.rows = rows
            self.next_offset = offset
            self.has_more = has_more
            self.scan_result = result
        return {"profiles": len(rows), "has_more": has_more}

    def enrich_merchants(self, rows, job: Job):
        """Attach org and merchant name. The status feed names a merchant but not its org, and
        the org is the level an operator plans a migration at."""
        wanted = sorted({row["merchant_id"] for row in rows} - self.merchants.keys())
        # Everything not already resolved is asked for again on every scan, which is how a
        # merchant that becomes readable turns back into a migratable profile.
        if wanted:
            job.progress(done=0, total=len(wanted), message=f"resolving {len(wanted)} merchants")
            counter = {"n": 0}
            counter_lock = threading.Lock()

            def resolve(merchant_id: str):
                entry = None
                try:
                    account = hs_call(
                        self.target, "GET", f"/accounts/{urllib.parse.quote(merchant_id)}", timeout=30
                    )
                    entry = {
                        "organization_id": account.get("organization_id"),
                        "merchant_name": account.get("merchant_name"),
                    }
                except UpstreamError:
                    # Rules can outlive the merchant account they were written for. Those
                    # profiles are reported as-is; a migration run would answer
                    # "merchant account could not be read" for them.
                    pass
                with counter_lock:
                    counter["n"] += 1
                    job.progress(done=counter["n"])
                return merchant_id, entry

            with ThreadPoolExecutor(max_workers=8) as pool:
                for merchant_id, entry in pool.map(resolve, wanted):
                    if entry is not None:
                        self.merchants[merchant_id] = entry

        for row in rows:
            entry = self.merchants.get(row["merchant_id"])
            row["organization_id"] = (entry or {}).get("organization_id")
            row["merchant_name"] = (entry or {}).get("merchant_name")
            row["merchant_resolved"] = entry is not None

    def check_scopes(self, rows, job: Job):
        """Ask the decision engine which of these profiles it actually holds a scope for.

        Nothing else in this tool can answer that. Hyperswitch's status endpoint reads the
        decision engine for rules alone — `routing_algorithm WHERE created_by = <profile>` —
        which never touches the `merchant_account` table the scope lives in. A profile with no
        scope answers with an empty rule list, indistinguishable from a scope that simply holds
        no rules yet: it reads `pending` before a migration and `migrated` after one, while
        `decide-gateway` would answer MERCHANT_NOT_FOUND for it and the SSO handoff TE_04.

        `reconcile` is read-only and writes nothing, so it runs on every scan.
        """
        if not self.decision_engine.public()["ready"]:
            for row in rows:
                row["scope_in_de"] = None
                row["ancestry_missing"] = False
            return None
        job.progress(message="checking decision engine scopes")
        try:
            # `reconcile` classifies on profile and merchant ids alone and writes nothing, so
            # the tree it is handed needs no names and no true organization — the placeholder
            # never reaches anything stored. Provisioning is Hyperswitch's job precisely because
            # it *does* store what it is given.
            report = de_call(
                self.decision_engine,
                "POST",
                DE_RECONCILE_PATH,
                body={
                    "orgs": [
                        {
                            "org_id": row.get("organization_id") or "org_unresolved",
                            "merchants": [
                                {
                                    "merchant_id": row["merchant_id"],
                                    "profiles": [{"profile_id": row["profile_id"]}],
                                }
                            ],
                        }
                        for row in rows
                    ],
                    # Every scope, not just the stranded ones, because a scope can exist with
                    # no ancestry recorded against it — provisioned before Hyperswitch synced
                    # the hierarchy, or registered by something that never had a tree to write.
                    # It costs response size and no extra reads: `reconcile` loads every
                    # merchant account either way and this only decides which it reports.
                    "include_all_scopes": True,
                },
            )
        except UpstreamError as err:
            # A scan that reached Hyperswitch is still worth having. Every profile is left
            # saying the scope was not read rather than claiming it is there, so nothing is cut
            # over on the strength of a check that did not happen.
            for row in rows:
                row["scope_in_de"] = None
                row["ancestry_missing"] = False
            return {"error": str(err)}

        missing = set(report.get("missing_in_de") or [])
        entries = report.get("scopes") or []
        # A scope the decision engine holds but has no ancestry against. It routes perfectly
        # well — `decide-gateway` needs only the row — but the dashboard cannot name the
        # organization or merchant above it, and a grant cannot expand to reach it. Tracked
        # apart from a missing scope because only the latter can block a cutover.
        no_ancestry = {
            entry.get("scope_id")
            for entry in entries
            if entry.get("classification") == "linked" and not entry.get("has_ancestry")
        }
        for row in rows:
            row["scope_in_de"] = row["profile_id"] not in missing
            row["ancestry_missing"] = row["profile_id"] in no_ancestry
        return {
            "checked_at": time.time(),
            "missing": sorted(missing),
            "no_ancestry": sorted(pid for pid in no_ancestry if pid),
            "linked": report.get("linked", 0),
            "unlinked": report.get("unlinked", 0),
            # Scopes whose id is an HS *merchant* id rather than a profile id. They hold live
            # rules and scores that no profile inherits, and this is the only place they are
            # visible, so they are carried through even though nothing here acts on them.
            "stranded": [
                entry.get("scope_id")
                for entry in entries
                if entry.get("classification") == "stranded"
            ],
        }

    @staticmethod
    def decorate(row):
        """Derive what the table sorts, filters and acts on, so the browser holds no rules of
        its own about what a state means."""
        missing = row.get("rules_missing_in_decision_engine") or []
        row["rules_pending"] = len(missing)
        row["rules_extra"] = len(row.get("rules_only_in_decision_engine") or [])
        # What "migrate selected" would actually move. A profile whose only rules are dynamic or
        # 3DS has nothing to carry, and one whose merchant account is gone cannot be attempted —
        # both would otherwise sit in `pending` forever and read as unfinished work.
        row["migratable"] = (
            row["rules_pending"] > 0
            and row.get("state") in UNFINISHED_STATES
            and row.get("merchant_resolved", False)
        )
        # Which direction, if any, a cutover may take this profile. `migrated` means every rule
        # is across and Hyperswitch is still deciding, which is exactly what a cutover changes.
        #
        # A profile the decision engine has no scope for is held back from that direction even
        # so: `decide-gateway` loads the `merchant_account` row before anything else and answers
        # MERCHANT_NOT_FOUND without one, so the override would move live traffic onto an engine
        # that refuses the profile. Rolling back stays offered in every case — for a profile
        # already cut over into that state, it is the way out.
        state = row.get("state")
        row["scope_missing"] = row.get("scope_in_de") is False
        # What a provisioning run would put right. A scope with no ancestry against it is not a
        # routing problem — only a naming and grant one — so it is worth a run without being
        # worth holding a cutover for.
        row["needs_provision"] = row["scope_missing"] or bool(row.get("ancestry_missing"))
        crossed = state in CUTOVER_FROM and not row["scope_missing"]
        row["cutover"] = "to_de" if crossed else ("to_hs" if state in ROLLBACK_FROM else None)
        if row.get("rules_hyperswitch", 0) == 0 and row.get("rules_out_of_scope", 0) > 0:
            row["blocker"] = "out_of_scope_only"
        elif not row.get("merchant_resolved", False):
            row["blocker"] = "no_merchant_account"
        else:
            row["blocker"] = None

    @staticmethod
    def totals(rows):
        totals = {
            "profiles": len(rows),
            "merchants": len({row["merchant_id"] for row in rows}),
            "organizations": len(
                {row.get("organization_id") for row in rows if row.get("organization_id")}
            ),
            "rules_hyperswitch": 0,
            "rules_decision_engine": 0,
            "rules_pending": 0,
            "rules_out_of_scope": 0,
            "migratable_profiles": 0,
            "cutover_ready": 0,
            "scopes_missing": 0,
            "scopes_no_ancestry": 0,
            "scopes_checked": 0,
            "states": {},
        }
        for row in rows:
            totals["rules_hyperswitch"] += row.get("rules_hyperswitch") or 0
            totals["rules_decision_engine"] += row.get("rules_decision_engine") or 0
            totals["rules_pending"] += row.get("rules_pending") or 0
            totals["rules_out_of_scope"] += row.get("rules_out_of_scope") or 0
            totals["migratable_profiles"] += 1 if row.get("migratable") else 0
            totals["cutover_ready"] += 1 if row.get("cutover") == "to_de" else 0
            totals["scopes_missing"] += 1 if row.get("scope_missing") else 0
            totals["scopes_no_ancestry"] += 1 if row.get("ancestry_missing") else 0
            totals["scopes_checked"] += 1 if row.get("scope_in_de") is not None else 0
            state = row.get("state", "unknown")
            totals["states"][state] = totals["states"].get(state, 0) + 1
        return totals

    # --------------------------------------------------------------- migrating

    def run_migrate(self, profile_ids, batch_size, job=None):
        job = job or self.migrate_job
        passes = [
            (profile_ids[i : i + batch_size], 0)
            for i in range(0, len(profile_ids), batch_size)
        ]
        # A profile holding more rules than one page needs its own run per page: the endpoint
        # reads `offset..offset+limit` of that profile's rules, and rules already carried are
        # skipped rather than rewritten.
        for profile_id, rule_count in self.rule_counts(profile_ids).items():
            for offset in range(RULE_PAGE_LIMIT, rule_count, RULE_PAGE_LIMIT):
                passes.append(([profile_id], offset))

        job.progress(done=0, total=len(passes), message=f"{len(passes)} batches queued")
        by_profile = {}
        order = []
        totals = {
            "profiles": 0,
            "rules_migrated": 0,
            "rules_skipped": 0,
            "rules_failed": 0,
            "rules_not_applicable": 0,
            "profiles_not_attempted": 0,
        }
        failures = []
        for index, (batch, offset) in enumerate(passes, start=1):
            try:
                response, request_id = hs_call(
                    self.target,
                    "POST",
                    "/routing/rule/migrate",
                    body={"profile_ids": batch, "limit": RULE_PAGE_LIMIT, "offset": offset},
                    with_request_id=True,
                )
                for profile in response.get("profiles", []):
                    merge_profile_result(by_profile, order, profile, request_id)
                for key in totals:
                    totals[key] += response.get("totals", {}).get(key, 0)
            except UpstreamError as err:
                # One batch failing says nothing about the rest, and the endpoint is idempotent,
                # so the run continues and reports the batch that did not land.
                failures.append(
                    {
                        "batch": index,
                        "profile_ids": batch,
                        "error": str(err),
                        "request_id": err.request_id,
                    }
                )
            job.progress(done=index, message=f"batch {index} of {len(passes)}")

        # Rule counters add up across passes because each pass carries different rules, but the
        # profile counters do not: a profile walked in four passes is still one profile.
        totals["profiles"] = len(order)
        totals["profiles_not_attempted"] = sum(
            1 for profile in by_profile.values() if profile.get("not_attempted")
        )
        # The table is stale the moment rules land, so the pages already read are re-read here
        # rather than leaving the operator looking at counts the migration just changed.
        job.progress(message="refreshing status")
        try:
            self.run_scan(pages=SCAN_PAGES_DEFAULT, restart=True)
        except Exception as err:
            failures.append({"batch": None, "profile_ids": [], "error": f"rescan failed: {err}"})

        return {
            "profiles": [by_profile[pid] for pid in order],
            "totals": totals,
            "failures": failures,
        }

    # ----------------------------------------------------------------- scopes

    def run_scope_provision(self, profile_ids):
        """Create the decision engine scopes these profiles have none of, through Hyperswitch.

        `/routing/rule/migrate` provisions the scope, with its ancestry, as the first thing it
        does for a profile — before it reads a single rule. Re-running it for a profile whose
        rules are already across therefore writes no rule and creates the scope, named with the
        organization, merchant and profile that only Hyperswitch can read: the profile name lives
        in `business_profile`, which is encrypted per merchant, and the owning merchant is the
        rule's processor rather than the provider the status feed reports.

        Building that tree here instead is what the decision engine's own `/admin/hierarchy/sync`
        would need, and this tool cannot do it without recording ancestry it knows to be wrong.
        """
        result = self.run_migrate(profile_ids, MIGRATE_BATCH_SIZE, self.scope_job)
        # Read from the rescan the run finishes with, so this is what the table now shows rather
        # than what the run hoped for.
        with self.lock:
            rows = {row["profile_id"]: row for row in (self.rows or [])}
        result["requested"] = len(profile_ids)
        result["still_missing"] = sorted(
            profile_id
            for profile_id in profile_ids
            if (rows.get(profile_id) or {}).get("scope_missing")
        )
        result["still_without_ancestry"] = sorted(
            profile_id
            for profile_id in profile_ids
            if (rows.get(profile_id) or {}).get("ancestry_missing")
        )
        return result

    # ---------------------------------------------------------------- cutover

    def run_cutover(self, profile_ids, source):
        """Point each profile's routing at `source` by writing a Superposition override."""
        job = self.cutover_job
        _, _, _, config_key = self.superposition.snapshot()
        to_de = source == SOURCE_DECISION_ENGINE
        engine = "Decision Engine" if to_de else "Hyperswitch Routing Engine"
        job.progress(done=0, total=len(profile_ids), message=f"{len(profile_ids)} profiles queued")

        results = {}
        counter = {"n": 0}
        counter_lock = threading.Lock()

        def put(profile_id: str, key: str):
            return superposition_call(
                self.superposition,
                "PUT",
                "/context",
                body={
                    "context": {"profile_id": profile_id},
                    "override": {key: source},
                    "description": f"Cut over profile {profile_id} to {engine}",
                    "change_reason": "DE cutover" if to_de else "DE rollback",
                },
            )

        def write(profile_id: str):
            entry = {"profile_id": profile_id, "ok": False, "error": None, "context_id": None,
                     "key_used": None}
            key = self.superposition.snapshot()[3]
            try:
                response = put(profile_id, key)
            except UpstreamError as err:
                alternate = alternate_key(key)
                if SCHEMA_REFUSAL not in err.body or alternate == key:
                    # One profile failing says nothing about the rest: each is its own override.
                    entry["error"] = str(err)
                    return finish(entry)
                try:
                    response = put(profile_id, alternate)
                except UpstreamError as second:
                    entry["error"] = f"{err} · under {alternate}: {second}"
                    return finish(entry)
                key = alternate
                self.superposition.adopt_key(key)
            entry["ok"] = True
            entry["key_used"] = key
            if isinstance(response, dict):
                entry["context_id"] = response.get("context_id") or response.get("id")
            return finish(entry)

        def finish(entry):
            with counter_lock:
                counter["n"] += 1
                job.progress(done=counter["n"], message=f"{counter['n']} of {len(profile_ids)} written")
            return entry

        # Four at a time: enough to move hundreds of profiles in a window, gentle enough that a
        # shared Superposition is not the thing that breaks during a migration.
        with ThreadPoolExecutor(max_workers=4) as pool:
            for entry in pool.map(write, profile_ids):
                results[entry["profile_id"]] = entry

        # The table is left as the last scan read it: the override is the thing that was asked
        # for, and re-reading the estate to watch it take effect costs a full status walk.
        ordered = [results[pid] for pid in profile_ids if pid in results]
        keys_used = {entry["key_used"] for entry in ordered if entry["key_used"]}
        return {
            "source": source,
            "config_key": config_key,
            # What the workspace actually took, which is not the configured name when it had no
            # schema for it and the other one landed.
            "key_used": sorted(keys_used),
            "key_switched": config_key if keys_used and config_key not in keys_used else None,
            "profiles": ordered,
            "totals": {
                "profiles": len(ordered),
                "written": sum(1 for e in ordered if e["ok"]),
                "failed": sum(1 for e in ordered if not e["ok"]),
            },
        }

    def ineligible_for(self, profile_ids, source):
        """Profiles the last scan says this direction does not apply to, with the state that
        makes them ineligible."""
        want = "to_de" if source == SOURCE_DECISION_ENGINE else "to_hs"
        with self.lock:
            rows = {row["profile_id"]: row for row in (self.rows or [])}
        blocked = {}
        for profile_id in profile_ids:
            row = rows.get(profile_id)
            if row is None:
                blocked[profile_id] = "not in the last scan"
            elif row.get("cutover") != want:
                # The state alone would read as a contradiction here — a `migrated` profile
                # refused a cutover to the decision engine — so the real reason is named.
                blocked[profile_id] = (
                    "no scope in the decision engine"
                    if want == "to_de" and row.get("scope_missing")
                    else row.get("state") or "unknown"
                )
        return blocked

    def not_provisionable(self, profile_ids):
        """Profiles the last scan says a provisioning run has nothing to do for, with why. The
        page is not taken on trust here for the same reason a cutover is not: a stale tab would
        otherwise run against a reconcile that has since been re-read."""
        with self.lock:
            rows = {row["profile_id"]: row for row in (self.rows or [])}
        blocked = {}
        for profile_id in profile_ids:
            row = rows.get(profile_id)
            if row is None:
                blocked[profile_id] = "not in the last scan"
            elif row.get("scope_in_de") is None:
                blocked[profile_id] = "the decision engine was not read"
            elif not row.get("needs_provision"):
                blocked[profile_id] = "already has a scope and its ancestry"
        return blocked

    def rule_counts(self, profile_ids):
        """Rules held per profile, from the last scan — both the kinds a migration carries and
        the kinds it does not, since the endpoint pages over all of them together."""
        with self.lock:
            scan = self.scan_result
        if not scan:
            return {}
        wanted = set(profile_ids)
        return {
            row["profile_id"]: (row.get("rules_hyperswitch") or 0)
            + (row.get("rules_out_of_scope") or 0)
            for row in scan["rows"]
            if row["profile_id"] in wanted
        }

    # ------------------------------------------------------------------- state

    def state(self, known_scan_at=None):
        """`known_scan_at` is the stamp of the scan the caller already holds. The scan is nearly
        all of this payload and changes only when one finishes, while the progress poll runs
        every 900ms for as long as a job does — so a caller that is already current is told to
        keep what it has instead of being sent the whole estate again."""
        with self.lock:
            scan = self.scan_result
            has_more = self.has_more
            pages_read = self.next_offset // STATUS_PAGE_SIZE
        unchanged = bool(scan) and scan["scanned_at"] == known_scan_at
        config = self.target.public()
        config.update(
            {
                "superposition": self.superposition.public(),
                "decision_engine": self.decision_engine.public(),
                "environments": self.environments.public(self.target.identity()[0]),
                "env_kinds": [kind.value for kind in EnvKind],
                "cutover_key_default": CUTOVER_KEY,
                "migrate_batch_size": MIGRATE_BATCH_SIZE,
                "status_page_size": STATUS_PAGE_SIZE,
                "saved": CONFIG_FILE.exists(),
            }
        )
        return {
            "config": config,
            "scan_job": self.scan_job.snapshot(),
            "migrate_job": self.migrate_job.snapshot(),
            "cutover_job": self.cutover_job.snapshot(),
            "scope_job": self.scope_job.snapshot(),
            "scan": None if unchanged else scan,
            "scan_unchanged": unchanged,
            "has_more": has_more,
            "pages_read": pages_read,
        }


class Handler(BaseHTTPRequestHandler):
    dashboard: Dashboard = None  # set in main

    def log_message(self, fmt, *args):  # one line per request, without the client noise
        print(f"[dashboard] {fmt % args}")

    def send_json(self, payload, status=200):
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = urllib.parse.urlparse(self.path).path
        if path in ("/", "/index.html"):
            page = (HERE / "index.html").read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            # The page is read from disk per request, so an edit is live on reload. Without this
            # the browser serves its own copy back and the edit appears not to have happened.
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(page)))
            self.end_headers()
            self.wfile.write(page)
        elif path == "/api/state":
            known = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query).get("scan_at")
            try:
                known_at = float(known[0]) if known else None
            except ValueError:
                known_at = None
            self.send_json(self.dashboard.state(known_at))
        else:
            self.send_json({"error": "not found"}, status=404)

    def do_POST(self):
        path = urllib.parse.urlparse(self.path).path
        length = int(self.headers.get("Content-Length") or 0)
        payload = json.loads(self.rfile.read(length) or b"{}") if length else {}
        board = self.dashboard

        if path == "/api/config":
            self.set_config(payload)
        elif path == "/api/environment":
            self.switch_environment(payload)
        elif path == "/api/scan":
            # A migration re-reads the status feed itself as its last step, so a scan started
            # now would race that one over the same merchant cache.
            if board.migrate_job.running or board.cutover_job.running or board.scope_job.running:
                self.send_json({"error": "a run is in flight, it refreshes the scan when it finishes"}, status=409)
                return
            pages = payload.get("pages", SCAN_PAGES_DEFAULT)
            pages = max(0, min(int(pages), 200))
            restart = bool(payload.get("restart"))
            if not board.scan_job.start(board.run_scan, pages, restart):
                self.send_json({"error": "a scan is already running"}, status=409)
                return
            self.send_json({"started": True})
        elif path == "/api/migrate":
            profile_ids = payload.get("profile_ids") or []
            if not profile_ids:
                self.send_json({"error": "no profiles selected"}, status=400)
                return
            if self.stopped_by_live_guard(payload, "writing rules"):
                return
            batch_size = int(payload.get("batch_size") or MIGRATE_BATCH_SIZE)
            if board.scan_job.running or board.scope_job.running:
                self.send_json({"error": "a scan is running, try again once it finishes"}, status=409)
                return
            started = board.migrate_job.start(
                board.run_migrate, profile_ids, max(1, min(batch_size, 1000))
            )
            if not started:
                self.send_json({"error": "a migration is already running"}, status=409)
                return
            self.send_json({"started": True, "profiles": len(profile_ids)})
        elif path == "/api/cutover":
            self.cutover(payload)
        elif path == "/api/de-scopes":
            self.create_scopes(payload)
        else:
            self.send_json({"error": "not found"}, status=404)

    def stopped_by_live_guard(self, payload, action: str) -> bool:
        """A production environment takes one more answer before anything is written to it: its
        own name, sent back with the request. The dashboard asks for it and holds it for the
        rest of the session, so a migration window is not a name typed per batch — but a client
        that never asked is stopped here rather than only in the browser."""
        name, label, kind = self.dashboard.target.identity()
        if not kind.live:
            return False
        if (payload.get("live_ack") or "").strip() == name:
            return False
        self.send_json(
            {
                "error": f"{label} is a production environment: {action} there needs its name "
                f"({name}) confirmed",
                "live_ack_required": name,
            },
            status=428,
        )
        return True

    def switch_environment(self, payload):
        """Point everything at another environment already configured here."""
        board = self.dashboard
        if (board.scan_job.running or board.migrate_job.running
                or board.cutover_job.running or board.scope_job.running):
            self.send_json({"error": "a job is running against the current environment"}, status=409)
            return
        name = (payload.get("name") or "").strip()
        record = board.environments.get(name)
        if record is None:
            self.send_json({"error": f"no environment named {name!r}"}, status=404)
            return
        board.use_environment(record)
        self.send_json(board.state())

    def create_scopes(self, payload):
        """Provision the decision engine scopes a set of profiles has none of."""
        board = self.dashboard
        profile_ids = payload.get("profile_ids") or []
        if not profile_ids:
            self.send_json({"error": "no profiles selected"}, status=400)
            return
        if self.stopped_by_live_guard(payload, "provisioning scopes"):
            return
        if not board.decision_engine.public()["ready"]:
            self.send_json(
                {"error": "set the decision engine URL and admin secret under Environment first"},
                status=400,
            )
            return
        if board.scan_job.running or board.migrate_job.running or board.cutover_job.running:
            self.send_json({"error": "a run is in flight, try again once it finishes"}, status=409)
            return
        blocked = board.not_provisionable(profile_ids)
        if blocked:
            sample = ", ".join(f"{pid} ({why})" for pid, why in list(blocked.items())[:5])
            self.send_json(
                {
                    "error": f"{len(blocked)} of {len(profile_ids)} profiles have nothing to "
                    f"provision: {sample}" + ("…" if len(blocked) > 5 else ""),
                    "blocked": blocked,
                },
                status=400,
            )
            return
        if not board.scope_job.start(board.run_scope_provision, profile_ids):
            self.send_json({"error": "a scope run is already running"}, status=409)
            return
        self.send_json({"started": True, "profiles": len(profile_ids)})

    def cutover(self, payload):
        """Point profiles at the decision engine, or back at Hyperswitch."""
        board = self.dashboard
        profile_ids = payload.get("profile_ids") or []
        source = payload.get("source") or SOURCE_DECISION_ENGINE
        if source not in (SOURCE_DECISION_ENGINE, SOURCE_HYPERSWITCH):
            self.send_json({"error": f"unknown routing source {source!r}"}, status=400)
            return
        if not profile_ids:
            self.send_json({"error": "no profiles selected"}, status=400)
            return
        if self.stopped_by_live_guard(payload, "switching routing"):
            return
        if not board.superposition.public()["ready"]:
            self.send_json({"error": "set the Superposition URL and secret under Environment first"}, status=400)
            return
        if board.scan_job.running or board.migrate_job.running or board.scope_job.running:
            self.send_json({"error": "a run is in flight, try again once it finishes"}, status=409)
            return
        # This switches live traffic, so the state each profile was last seen in decides whether
        # it may be switched at all — the browser's view of it is not taken on trust.
        if not payload.get("force"):
            blocked = board.ineligible_for(profile_ids, source)
            if blocked:
                sample = ", ".join(f"{pid} ({why})" for pid, why in list(blocked.items())[:5])
                self.send_json(
                    {
                        "error": f"{len(blocked)} of {len(profile_ids)} profiles are not in a state "
                        f"this direction applies to: {sample}"
                        + ("…" if len(blocked) > 5 else ""),
                        "blocked": blocked,
                    },
                    status=400,
                )
                return
        if not board.cutover_job.start(board.run_cutover, profile_ids, source):
            self.send_json({"error": "a cutover is already running"}, status=409)
            return
        self.send_json({"started": True, "profiles": len(profile_ids)})

    def set_config(self, payload):
        board = self.dashboard
        if (board.scan_job.running or board.migrate_job.running
                or board.cutover_job.running or board.scope_job.running):
            self.send_json({"error": "a job is running against the current environment"}, status=409)
            return
        hs_url = (payload.get("hyperswitch_url") or "").strip()
        if not hs_url.startswith(("http://", "https://")):
            self.send_json({"error": "the URL needs an http:// or https:// scheme"}, status=400)
            return
        headers = {
            (entry.get("name") or "").strip(): (entry.get("value") or "").strip()
            for entry in (payload.get("headers") or [])
            if (entry.get("name") or "").strip()
        }
        # The other two addresses are checked before anything is applied, so a typo in one of
        # them cannot leave the console pointed half at the new environment and half at the old.
        for field, what in (("decision_engine", "decision engine"), ("superposition", "Superposition")):
            section = payload.get(field)
            url = (section.get("url") or "").strip() if isinstance(section, dict) else ""
            if url and not url.startswith(("http://", "https://")):
                self.send_json({"error": f"the {what} URL needs an http:// or https:// scheme"}, status=400)
                return
        # An empty key field means "leave the key alone" while the environment stays the same,
        # so editing a header does not force the key to be typed again. Pointing somewhere else
        # drops it: a key belongs to one environment, and carrying it over would send the last
        # environment's credential to the new host.
        previous_url, previous_key, _ = board.target.snapshot()
        previous_name, previous_label, previous_kind = board.target.identity()
        # The panel either edits the environment on screen or adds one beside it, which is what
        # the browser says here rather than something guessed from the fields: the same typing
        # renames an environment in the first case and creates one in the second.
        creating = bool(payload.get("create"))
        label = (payload.get("label") or "").strip() or ("" if creating else previous_label)
        wanted = (payload.get("name") or "").strip() or slug_of(label or host_of(hs_url))
        # One name is one place to look for one set of credentials, so a name the store already
        # holds is refused rather than written over — the picker is how you reach that one.
        if (creating or wanted != previous_name) and board.environments.get(wanted) is not None:
            self.send_json(
                {"error": f"an environment named {wanted!r} is already configured — pick it "
                          f"from the header to work on it"},
                status=409,
            )
            return
        if creating:
            # Parked before anything is overwritten, so the environment being left keeps the
            # addresses and credentials it had.
            board.remember_current(previous_name)
        # A credential belongs to one environment: it does not follow the panel to another
        # address, and never into an environment that is being added.
        moved = creating or hs_url != previous_url
        api_key = (payload.get("api_key") or "").strip() or ("" if moved else previous_key)
        board.target.set(
            hs_url,
            api_key,
            headers,
            label,
            (payload.get("name") or "").strip(),
            # What the environment says it is stands while it points at the same place. A URL
            # typed over the old one is a different environment, and is read from its address —
            # which is how a production URL pasted into an empty panel comes out marked as one.
            payload.get("kind") or (guess_kind(hs_url) if moved else previous_kind),
        )
        if moved:
            board.forget_scan()

        de = payload.get("decision_engine")
        if isinstance(de, dict):
            de_url = (de.get("url") or "").strip()
            previous_de_url, previous_de_secret, _ = board.decision_engine.snapshot()
            board.decision_engine.set(
                de_url,
                (de.get("admin_secret") or "").strip()
                or ("" if creating or de_url != previous_de_url else previous_de_secret),
                {
                    (entry.get("name") or "").strip(): (entry.get("value") or "").strip()
                    for entry in (de.get("headers") or [])
                    if (entry.get("name") or "").strip()
                },
            )

        sp = payload.get("superposition")
        if isinstance(sp, dict):
            sp_url = (sp.get("url") or "").strip()
            previous_sp_url, previous_secret, _, _ = board.superposition.snapshot()
            secret = (sp.get("secret") or "").strip() or (
                "" if creating or sp_url != previous_sp_url else previous_secret
            )
            board.superposition.set(
                sp_url,
                secret,
                {
                    (entry.get("name") or "").strip(): (entry.get("value") or "").strip()
                    for entry in (sp.get("headers") or [])
                    if (entry.get("name") or "").strip()
                },
                sp.get("config_key") or "",
            )
        # Held in the store under whatever it is now called, so the picker offers it and its
        # credentials are still here after a look at another environment.
        board.remember_current("" if creating else previous_name)
        try:
            if payload.get("remember"):
                save_config(
                    board.environments,
                    board.target.identity()[0],
                    bool(payload.get("remember_key")),
                )
            elif payload.get("forget"):
                forget_config()
        except OSError as err:
            self.send_json({"error": f"config not saved: {err}"}, status=500)
            return
        self.send_json(board.state())


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--hs-url", default=os.environ.get("HS_URL", ""))
    parser.add_argument(
        "--admin-api-key",
        default=os.environ.get("HS_ADMIN_API_KEY", ""),
        help="admin API key; both migration endpoints are admin-authenticated. Optional — it "
        "can be pasted into the dashboard instead, which is where a hosted key usually comes from",
    )
    parser.add_argument(
        "--de-url",
        default=os.environ.get("DE_URL", ""),
        help="decision engine base URL. Optional — without it the dashboard reports rules only "
        "and says the scope column was not read",
    )
    parser.add_argument("--de-admin-secret", default=os.environ.get("DE_ADMIN_SECRET", ""))
    parser.add_argument(
        "--env",
        default=os.environ.get("HS_ENV", ""),
        help="name of the environment to open on, e.g. sandbox or production-eu. Without it, "
        "whichever one was last in use",
    )
    parser.add_argument("--port", type=int, default=int(os.environ.get("PORT", "8090")))
    parser.add_argument("--host", default=os.environ.get("HOST", "127.0.0.1"))
    args = parser.parse_args()

    saved = load_saved() or {}
    environments = Environments(saved.get("environments") or [])
    environments.seed(ENV_PRESETS)
    if args.env and environments.get(args.env) is None:
        parser.error(
            f"no environment named {args.env!r}; known: "
            + ", ".join(record["name"] for record in environments.records_list())
        )
    record = (
        environments.get(args.env)
        or environments.get(saved.get("active") or "")
        or environments.get(ENV_PRESETS[0]["name"])
    )
    # A URL on the command line is the environment to open on: the saved one it belongs to if
    # there is one, so its credentials come with it, and otherwise an environment of its own.
    if args.hs_url:
        hs_url = args.hs_url.rstrip("/")
        record = next(
            (r for r in environments.records_list() if r["hyperswitch_url"] == hs_url),
            normalise_env({"hyperswitch_url": hs_url}),
        )
    if args.admin_api_key:
        record["api_key"] = args.admin_api_key
    if args.de_url:
        record["decision_engine"]["url"] = args.de_url
    if args.de_admin_secret:
        record["decision_engine"]["admin_secret"] = args.de_admin_secret

    target, superposition, decision_engine = Target(""), Superposition(), DecisionEngine()
    apply_env(record, target, superposition, decision_engine)
    environments.remember(env_record(target, superposition, decision_engine))
    Handler.dashboard = Dashboard(target, superposition, decision_engine, environments)
    server = ThreadingHTTPServer((args.host, args.port), Handler)
    name, label, kind = target.identity()
    hs_url, api_key, _ = target.snapshot()
    print(f"migration dashboard  http://{args.host}:{args.port}")
    print(f"  environment        {label} [{name}]" + ("  ** production **" if kind.live else ""))
    print(f"  hyperswitch        {hs_url}")
    print(f"  api key            {'set' if api_key else 'not set — paste it in the dashboard'}")
    print(f"  superposition      {superposition.public()['url'] or 'not set — paste it in the dashboard'}")
    print(f"  decision engine    {decision_engine.public()['url'] or 'not set — scopes will not be checked'}")
    print("  others             " + ", ".join(
        r["name"] for r in environments.records_list() if r["name"] != name))
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nstopped")


if __name__ == "__main__":
    main()
