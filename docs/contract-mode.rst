Contract Mode
=============

Contract mode (``ingot --contract <contract.toml>``) turns a keyed data model into the tables a
key-value store core reads, a machine-readable contract, a keyspace declaration for tooling, and,
when the model maps keys to a device shadow, the tables a shadow binder, reporter and command
router walk. Every rule below is checked at generation time: a model that breaks any of them
generates nothing, and every refusal is logged with the offending item. ``examples/contract/``
holds a generic model at contract 1.0, 1.1 and 2.0.

Inputs
------

``contract.toml``
   The contract (``[contract]``: ``name``, ``major``, ``minor``, ``patch``, ``namespace_dir``,
   optional ``variants`` and ``units``), the queue and staging ``[bounds]``, ``[[reserved]]`` IDs,
   ``[[principals]]`` with their ``grants``, ``[[uid_caps]]``, ``[[error_groups]]``, and the
   contract-level shadow rows: ``[[apply_groups]]``, ``[[shadow_echo]]``,
   ``[[shadow_constant]]``, ``[shadow_rejected]``, ``[[topics]]``, ``[[topic_payload]]`` and
   ``[[placeholders]]``.

Namespace files (every ``.toml`` in ``namespace_dir``)
   ``[meta]`` (``id``, ``namespace_id``, ``mfg``), ``[enums.<name>]``, ``[[classes]]`` with their
   ``[[classes.keys]]`` and ``[[classes.commands]]``, and the namespace's ``[[shadow_commands]]``.

``units.toml`` (when ``units`` names it)
   The unit vocabulary: token, quantity and meaning.

Every input file enters the model hash. ``--variant <name>`` selects one of the declared
variants; when variants are declared it is required.

Keys, classes and commands
--------------------------

A key's ID encodes its namespace, class, index and type. A class has a role (``settings``,
``state``, ``command``, ``config``, ``diag``) and a default persistence (``none``, ``deferred``,
``durable``). A key declares its ``type``, ``introduced_in`` minor, and optionally ``write_rate``,
``rate_burst``, ``persistence``, ``deferred_ms``, ``secret`` with its ``readers``,
``survives_reset``, ``alias_of``, ``default`` (``defaults`` per variant), ``enum``, ``max_size``,
``wk`` (a well-known role), ``doc``, ``variants`` (the variants it exists on), and an ``error``
bit. A command type (``[[classes.commands]]``) creates its command, result and dedupe keys with
its owner, payload kind and sizes; ``internal`` says why it is never requested on the wire.
``outcome = "<enum>"`` names an enum of the command's namespace whose values are the codes its
result's outcome field carries (``DMT_RESULT_HEADER_BYTES``: epoch, sequence, outcome); it changes
no table, only ``dmt_constants.h`` and ``contract.json``:

.. code-block:: toml

   [enums.drive_outcome.values]
   accepted = 0
   refused = 1
   busy = 2

   [[classes.commands]]
   id = "drive"
   # ... indices, owner, sizes ...
   outcome = "drive_outcome"   # DMT_E_DRIVE_COMMAND_DRIVE_ACCEPTED 0, ..._REFUSED 1, ..._BUSY 2

Numeric keys may carry a ``unit`` (a token of the vocabulary), an inclusive ``min``/``max`` with
``step``, or a discrete ``allowed`` set, a ``scale``, and an ``unset`` sentinel default outside the
range.

Enums
-----

``[enums.<name>.values]`` maps a member name to its code, or to ``{ value, wire, accept }``:
``wire`` is the string the member is reported as, ``accept`` lists further inbound strings. A
member may instead declare ``report_as = "<member or wire string>"`` to report another member's
string (many codes to one string outbound). ``[enums.<name>.variants.<v>]`` removes members and
adds or renames values for one variant. Within a resolved enum every code, name and inbound
string is unique.

Principals and rights
---------------------

A principal has a name, ``uid``, ``token`` (0 for the uid itself), ``kind`` (``unix``,
``inproc``, ``uart``), flags, heartbeat, budgets and caps, ``introduced_in`` and ``grants``
(a namespace, class or key with ``r``, ``w`` or ``rw``; the last grant on a key wins). The rights
table, per-principal handshake digests (KSET v1, one per contract minor) and queue bounds follow.
``dmt_principal_names`` holds every principal's declared name, in table order, never ``NULL``.

Error groups
------------

``[[error_groups]]`` declare a word of error bits: ``width``, ``wire_base`` (per variant in
``wire_bases``), ``writer``, ``reserved_bits``, ``publish_max_bit`` and the shadow path of the
error list. A bool STATE key with ``error = { group, bit }`` is one error; its wire id is
``e<wire_base + bit>``.

Shadow mapping
--------------

Paths are dotted identifiers: each dot is one nested JSON object.

Key paths
   ``shadow`` on a SETTINGS key is a desired path; on a STATE or CONFIG key it is a reported
   path. It may be a list, and each entry may be ``{ path, as, scope, variants }``: ``as =
   "integer"`` carries an enum code as a number (inbound and outbound), or a bool as ``0``/``1`` on
   a reported path (inbound a bool stays ``true``/``false``), ``scope`` (``full``, ``delta``,
   ``both``) places a reported path in the full document, the on-change delta or both,
   ``variants`` limits the entry. One path may be listed once per variant and document, so a key
   may report one path as ``0``/``1`` in the full document and as ``true``/``false`` in the delta
   (``DSH_FORM_BOOL_INTEGER``, shadow table interface 1.2). Further key attributes: ``applies`` (the SETTINGS key
   a STATE twin reports), ``apply_group`` and ``implies`` (coupled writes in one batch),
   ``on_reject`` (``echo`` or ``clear_desired``), ``inbound_unknown`` and ``outbound_unknown`` (what
   an unknown string or code becomes: a wire string, ``ignore``/``reject`` inbound, ``raw``
   outbound), ``forced`` (re-emitted on every delta), ``render`` (an index list reported as wire
   strings) and ``telem`` (a telemetry name).

``[[shadow_echo]]``
   A desired path reported back as received, with no key: ``path``, ``type``, ``max_size``,
   ``command_key`` (the SETTINGS key a command body applies it to), ``variants`` and ``scope``.
   One echo row may share its delta member with a reported key of the same JSON value.

``[[shadow_constant]]``
   A reported path with a literal ``value`` or an ``image`` value (the contract version, or the
   hash of the resolved keyspace body), optionally ``forced``.

``[shadow_rejected]``
   ``path``: the reported path where the binder lists the desired members it rejected in the
   update it answers. It is a delta row (``DSH_RK_REJECTED``, shadow table interface 1.3) with no
   key: the binder builds the list from the rows it rejected. ``variants`` limits it to those
   variants (a variant not listed reports no list). Absent, no list is reported. Its
   path is checked against the other delta rows, a model with no desired path may not declare
   it, and it is kept unchanged within a major.

``[[topics]]``
   ``id``, ``template`` over ``{stage}``, ``{duid}``, ``{model}``, ``{thing}`` and
   ``{shadow_name}``, ``direction`` (``pub`` or ``sub``), ``connection``, ``subscribe_order``,
   ``local_wrapper``, ``ack`` (the response topic), ``envelope`` (the request ``id`` field, required
   or with an absent value, and the ``source`` field with its enum and absent member),
   ``settings_in_every_body`` and ``literal_shadow_name``. A topic may declare ``vars = [{ name,
   max_len }]``: per-message placeholders, each ``{name}`` in the template, at most 8 per topic
   and each up to 128 bytes (``dtp_vars``, shadow table interface 1.4). A pub topic's publisher
   fills them; on a sub topic (interface 1.6) each var is one whole topic level, used once, which
   the consumer subscribes to as a single-level wildcard and extracts from a received topic. A
   template placeholder that is neither per-device nor declared is refused, and so is a declared
   var the template does not use. ``variants`` limits a topic to those variants: a
   variant not listed has no row, and a topic is compared on a previous contract's variant (a
   minor may add variants, not drop or change the topic there).

``[[placeholders]]``
   ``name`` and ``max_len``: a named value the consumer is configured with (a build or device
   value), usable as ``{name}`` in any topic template; at most 8, each up to 128 bytes, each used
   by some topic, and no topic var may take its name (``dtp_placeholders``, shadow table interface
   1.7). They are kept unchanged within a major.

``[[topic_payload]]``
   One member of a pub topic's payload: ``topic``, ``path``, and one of ``key`` (any key with a
   JSON value), ``value`` or ``image``; an enum member takes ``as`` or declares its
   ``outbound_unknown``; ``variants`` limits it.

``[[shadow_commands]]`` (in namespace files)
   How a request reaches the store: ``topic``, ``selector`` (``field`` equal to a ``wire`` string or
   flag, an array ``element``, or an ``object``), ``body`` (``fields``, ``settings`` or ``{ object,
   selector }``), ``protocol`` (``single`` or ``command_array``), and then either a ``command`` with
   ``args`` and further commands in ``also``, an ``answer`` code, an ``internal`` reason (with an
   optional ``internal_id``, 1 to 65535: the model's stable id of the action, which the library
   dispatches on instead of the wire name; ``dsh_command_t.internal_id``, shadow table interface
   1.5), or args that write STATE keys only. An argument takes a request ``field`` (optionally an ``element`` and
   an ``encoding``: ``base64`` into a string, binary or struct key, ``decimal_text`` into an integer
   key) or a constant ``value``, with ``required`` and its ``ec``, an ``absent`` value, or
   ``omit_when_absent``. ``variants`` with ``grounded_in`` limits a row.

Outputs
-------

``dmt_types.h``
   The fixed table interface (TIV) of the store tables.

``dmt_tables.h``, ``dmt_tables.c``
   Keys, defaults, reserved IDs, principals, rights, caps, commands, well-known roles and
   digests of the selected variant.

``dmt_constants.h``
   The code of every value of every enum key as ``#define DMT_E_<KEY PATH>_<VALUE> <code>`` for the
   selected variant (upper case, dots as underscores), so no consumer writes an enum code by hand,
   and of every command's ``outcome`` enum as ``#define DMT_E_<COMMAND PATH>_<VALUE> <code>``,
   placed at the command key's ID. It is per model, outside the table interface; a name that is
   not a C identifier, is longer than 63 characters, or is produced by two (key or command, value)
   pairs is refused. Constants are kept within a major, as enum values and outcomes are.

``contract.json``
   Every key, principal, right, command (with its ``outcome`` names and codes, when it has any)
   and shadow row, read back by ``--previous-contract``.

``keyspace.json``, ``keyspace.<variant>.json`` (with ``.json.gz`` and ``.head.json``), ``keyspace.schema.json``
   The declaration for tooling: all variants with per-variant overrides, the resolved variant,
   its deterministic gzip body and head (size and SHA-256), and the JSON schema. No values and no
   secret values appear.

``dsh_types.h``, ``dsh_tables.h``, ``dsh_tables.c`` (when the model maps the shadow)
   The fixed shadow table interface and the variant's binder (``dsh_desired``, ``dsh_echo``,
   enums with inbound and outbound strings, coupled writes, discrete sets), reporter
   (``dsh_reported`` in emission order), error-group, command-router (``dsh_commands``,
   ``dsh_steps``, ``dsh_args``) and topic (``dtp_topics``, ``dtp_payload``) tables, the contract
   hash, and the worst-case full and forced-delta document sizes. Shadow table interface 1.8 adds
   the bounds a consumer sizes its buffers with: ``DSH_REJECTED_MAX_ITEMS`` (one member per
   desired and echo path; 0 without a rejected-settings list), ``DSH_DESIRED_PATH_MAX_BYTES`` (the
   longest desired or echo path with its terminator), ``DSH_ERROR_IDS_PUBLISHED`` (every group's
   bits up to ``publish_max_bit`` that are not reserved) and ``DSH_ERROR_ID_MAX`` (the largest wire
   id of a published or declared bit). Interface 1.9 adds ``DSH_DESIRED_VALUE_MAX_BYTES``: 1 plus
   the longest JSON text of a value any ``dsh_desired`` or ``dsh_echo`` row accepts (0 with no
   row), each counted at the longest text the document size bound uses for its type: a bool 5, an
   integer type its longer extreme in decimal, a float32 47 (``%f`` of ``-FLT_MAX``), and a JSON
   string that unescapes to *n* bytes 2 + 6\ *n* (every byte as ``\u00XX``). A string row accepts
   up to ``max_size`` bytes; an enum wire-string row accepts its wire strings and accept aliases
   (an unknown string its ``inbound_unknown`` rule maps is not counted); an enum integer row and
   an echo row count by their type (an ``int`` echo as int32). A desired value the binder
   refuses can be any text the sender wrote, so a consumer listing refused values leaves out one
   longer than ``DSH_DESIRED_VALUE_MAX_BYTES - 1``.

``sdk/<principal>/`` (with ``--sdk-binding <DIR>``)
   Each file of the binding directory rendered once per principal, named after the file without a
   trailing ``.tera``; see the CLI reference for the naming rule and the context. Ingot names no
   store runtime.

``release-review.txt`` (with ``--previous-contract``)
   New principals' sensitive grants, for the release review.

Versions
--------

Within a major a contract only grows: keys, principals and rights are added at a new minor,
existing keys keep their ID, persistence, limits (ranges and sets may widen), enum values and
well-known roles, a command keeps its outcomes (it may gain outcomes, or name them for the first
time), error bits stay, and the shadow mapping only grows: a mapped key keeps its path
entries and wire attributes, its paths and accept aliases may be added to, and every echo,
constant, topic, command and payload row stays as it was. A major reserves the IDs of removed keys.
``--previous-contract`` checks all of this against the previous release's ``contract.json``.

Refusals
--------

Every rule above has a refusal, grouped as follows:

* **Layout and IDs:** duplicate namespace IDs, names or encoded IDs, a reserved or zero ID, a
  key index out of range, a type that cannot hold its default or size.
* **Values:** a default outside its type, enum, range or set; an unknown unit; a scale without a
  unit; a range or set on a non-numeric or enum key; a key ``enum`` or command ``outcome`` naming
  no enum of its namespace; an enum or outcome constant that is not a C identifier, longer than
  63 characters, or named twice.
* **Variants:** an undeclared ``--variant`` or variant named anywhere; an enum variant that
  removes an unknown name or makes a code, name or string ambiguous.
* **Rights and principals:** grants on unknown keys or scopes, a secret's reader that is not a
  principal, a heartbeat with a second writer, duplicate or reserved principal names.
* **Error groups:** a non-bool or non-STATE error key, a bit outside the width or reserved, two
  keys on one bit, overlapping wire ids.
* **Shadow:** a malformed path, two rows on one path in one document, a shadowed SETTINGS key
  without an applied twin, an enum wire string missing, an unknown rule missing or misplaced, a
  form or scope on the wrong role, a forced key without a delta path, a full document above
  8192 bytes.
* **Topics and payloads:** a placeholder outside the set, a duplicate id, template, order or
  wrapper, an ack that is not a pub topic, a payload member on a sub topic, without exactly one
  source, on an unknown or binary key, or with a misplaced form or unknown rule.
* **Command rows:** an unknown topic or command, a request selected twice on one variant, a
  selector with no or several forms, a field that is not dotted identifiers, an argument key
  outside the command's class that is not a STATE key, an argument value outside its key's enum
  or type, a misplaced encoding, an ``answer`` or ``internal`` with a command or args, ``also``
  without a command, and a command of the variant neither on the wire nor internal.
* **Versions:** anything a minor may not change (see Versions).
