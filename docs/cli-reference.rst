CLI Reference
=============

::

    ingot -m <PATH>... [OPTIONS]

Required
--------

====================================  ==========================================
Option                                Description
====================================  ==========================================
``-m, --model <PATH>``                Path to TOML data model file(s) or
                                      directory of TOML files. Repeatable —
                                      multiple ``-m`` flags merge all inputs
                                      into a single data model.
====================================  ==========================================

Output & Target
---------------

+------------------------+----------------+--------------------------------------------------+
| Option                 | Default        | Description                                      |
+========================+================+==================================================+
| ``-o, --output <DIR>`` | ``generated/`` | Output directory for generated code              |
+------------------------+----------------+--------------------------------------------------+
| ``-t, --target <T>``   | ``linux64``    | Target platform (see below)                      |
+------------------------+----------------+--------------------------------------------------+
| ``--no-events``        | off            | Disable event callback generation                |
+------------------------+----------------+--------------------------------------------------+
| ``--emit-tinyfsm``     | off            | Emit C++/tinyfsm event types and dispatch        |
+------------------------+----------------+--------------------------------------------------+
| ``--templates <DIR>``  | built in       | Replace the built-in templates with a directory  |
+------------------------+----------------+--------------------------------------------------+
| ``--table-types``      | off            | Write only the fixed table headers (no model)    |
+------------------------+----------------+--------------------------------------------------+

The templates are compiled into the binary, so a given ingot build always
generates the same output. ``--templates`` (or the ``INGOT_TEMPLATES_DIR``
environment variable) replaces them with a directory, for template development.
``--table-types`` writes ``dmt_types.h`` and ``dsh_types.h``, the table-interface headers, which do
not depend on any model.
``--emit-tinyfsm`` applies only to C targets. It emits additive C++ headers and
source for consumers using tinyfsm.

Contract Mode
-------------

+--------------------------------+---------+--------------------------------------------------+
| Option                         | Default | Description                                      |
+================================+=========+==================================================+
| ``--contract <FILE>``          | —       | Generate the table-interface store tables from a |
|                                |         | ``contract.toml`` and its namespace files        |
+--------------------------------+---------+--------------------------------------------------+
| ``--previous-contract <FILE>`` | —       | The previous release's ``contract.json``; checks |
|                                |         | that a minor is additive and a major reserves    |
|                                |         | removed IDs, and writes ``release-review.txt``   |
+--------------------------------+---------+--------------------------------------------------+
| ``--allow-dirty-ingot``        | off     | Accept an ingot built from a dirty tree (for     |
|                                |         | ingot's own tests); the output says ``-dirty``   |
+--------------------------------+---------+--------------------------------------------------+
| ``--shadow-buffer <BYTES>``    | —       | Warn when the full shadow document can exceed    |
|                                |         | the shadow composer's buffer                     |
+--------------------------------+---------+--------------------------------------------------+
| ``--sdk-binding <DIR>``        | —       | Templates rendering a per-principal SDK against  |
|                                |         | the consumer's store runtime                     |
+--------------------------------+---------+--------------------------------------------------+

Contract mode writes ``dmt_types.h``, ``dmt_tables.h``, ``dmt_tables.c``,
``dmt_constants.h`` (the enum value constants of every enum key, and of every command that names
its result codes with ``outcome = "<enum>"``, once per enum that a topic var names with
``enum = "<namespace>::<enum>"`` as ``DMT_E_<NAMESPACE>_<ENUM>_<VALUE>``, and for every struct key with a ``header`` its
``DMT_<KEY>_HEADER_BYTES``, ``_<FIELD>_OFFSET`` and ``_PAYLOAD_MAX`` constants, its
``dmt_<key>_header_t`` and its inline byte-shift ``dmt_<key>_header_encode`` /
``dmt_<key>_header_decode``) and ``contract.json`` for the selected ``--variant``, or nothing at all if the model
breaks any rule; every refusal is logged. It always uses the built-in templates.
Each ``dmt_keys`` row and each ``contract.json`` key carries ``rate_per_day`` and ``rate_burst``
from its ``write_rate`` and ``rate_burst`` (else ``[bounds] default_rate_burst``); a command
type's pair applies to its command, result and dedupe keys alike, and a ``rate_burst`` without a
``write_rate`` is refused. ``--previous-contract`` lets a minor change either.
A model that maps keys to the shadow also gets ``dsh_types.h`` (the fixed shadow
table interface) and ``dsh_tables.h`` / ``dsh_tables.c``: the binder, reporter,
command-router, error-group and topic tables of the variant, and the bounds a shadow consumer
sizes its buffers with (interface 1.9: rejected-list members, the longest desired path and
desired value text, published error ids and the largest error id; see Contract Mode), and
(interface 1.10) a ``DSH_INTERNAL_<VERB>`` constant for each internal row's ``internal_id``. With ``--sdk-binding <DIR>``, every
principal gets an SDK in ``sdk/<principal>/`` (dots in the name become underscores): each file of
the binding directory is a template rendered once per principal, under its file name with a
trailing ``.tera`` removed (``x_PRINCIPAL.c.tera`` and ``x_PRINCIPAL.c`` both render to
``x_<principal>.c``) and ``PRINCIPAL`` replaced by the principal's identifier; two templates with
one output name, or an empty one, are refused. The context carries the principal's
identity (``principal``, ``p``, ``kind``, ``uid``, ``token``, ``keyset_ids``, ``keyset_types``,
``digest``), the contract (``name``, ``major``, ``minor``, ``patch``, ``variant``,
``model_hash``), the typed ``accessors`` and command ``submits`` its rights give, helper
flags, and ``value_headers``: the struct value headers of the keys it may read or write, each with
``path``, ``prefix`` (``DMT_<KEY>``), ``ctype``, ``encode``, ``decode``, ``bytes``,
``payload_max`` and ``fields`` (``name``, ``offset_name``, ``ctype``, ``offset``, ``size``,
``kind``), named as in ``dmt_constants.h``; and ``views``: one per struct key it may read
(read-only or read-write, command keys included), in key ID order, each with ``name``
(``namespace_class_key``), ``path`` and ``key`` (the key ID as ``0x%08XU``). A struct value
reaches a reader only as a delivered event and has no getter, so the binding renders each view as
a function of an event, for example ``<p>_view_<name>(ev, &data, &len)``, that gives the event's
whole value (a declared header included; decoding it stays the consumer's) when the event is that
key and carries a buffer, and the runtime's type mismatch otherwise: the consumer tells which key
an event carries without writing a key ID. A principal without such a key has an empty
``views``. The binding names the consumer's store runtime; ingot names none.
``examples/contract/`` holds a generic model at contract 1.0, 1.1 and 2.0.

Key Filtering
-------------

=====================================  ==========================================
Option                                 Description
=====================================  ==========================================
``--include-list <FILE>``              YAML list of keys to include (whitelist);
                                       all others are excluded
``--exclude-list <FILE>``              YAML list of keys to exclude (blacklist);
                                       all others are included
``--persistent-keys <FILE>``           YAML list of keys that should be marked
                                       persistent
``--property-override-list <FILE>``    YAML map of per-key property overrides
                                       (``KEY: {default_value: val}``)
=====================================  ==========================================

``--include-list`` and ``--exclude-list`` are mutually exclusive.

Key names in filter lists are matched with underscore-normalized comparison,
so both ``STATEOFCHARGE`` and ``STATE_OF_CHARGE`` will match the same key.

Variant Overrides
-----------------

=========================  ==========================================
Option                     Description
=========================  ==========================================
``--variant <NAME>``       Product variant for per-variant default
                           overrides (e.g. ``peabodyv0``,
                           ``omnidrive_v2``). Applies variant-specific
                           defaults from ``[classes.keys.defaults]``
                           and merges variant enum values from
                           ``[enums.*.variants.*]``.
=========================  ==========================================

Verbosity
---------

+-----------------------------+---------+----------------------------------+
| Option                      | Default | Description                      |
+=============================+=========+==================================+
| ``-v`` / ``-vv`` / ``-vvv`` | warn    | Verbosity: info / debug / trace  |
+-----------------------------+---------+----------------------------------+


Targets
-------

===============  ==================  ====================  ==============================
Value            Platform            Mutex                 Notes
===============  ==================  ====================  ==============================
``stm32``        32-bit ARM STM32    None (bare-metal)     4-byte alignment
``esp-xtensa``   ESP32 Xtensa        FreeRTOS semaphore    ``xSemaphoreTake``/``Give``
``esp-riscv``    ESP32 RISC-V        FreeRTOS semaphore    ``xSemaphoreTake``/``Give``
``mcu8bit``      8-bit MCU           None (bare-metal)     1-byte alignment, 16-bit pointers
``linux64``      64-bit Linux        pthread mutex         ``PTHREAD_MUTEX_INITIALIZER``
``rust``         no_std Rust module  Caller-managed        Generates ``dm.rs``
===============  ==================  ====================  ==============================

Thread-safe keys are protected by the target's mutex mechanism. Keys without
``thread_safe = true`` bypass locking entirely.

The ``rust`` target generates a standalone no_std Rust module using static
perfect-hash storage. It provides named accessors, scalar access by encoded
key, and ``PERSISTENT_KEYS`` metadata. Persistence I/O and mutex handling are
left to the consumer. C-only options such as ``--no-events`` and
``--emit-tinyfsm`` have no effect for Rust.


Examples
--------

Generate from a single model file::

    ingot -m device.toml -o src/dm/ -t stm32

Merge a directory of namespace TOML files::

    ingot -m models/ -o gen/udm -t linux64

Merge files from multiple locations::

    ingot -m common/base.toml -m project/overrides.toml -o gen/

Use include list and persistent keys::

    ingot -m models/ --include-list conf/include.yml \
        --persistent-keys conf/persist.yml -o gen/

Apply per-key property overrides::

    ingot -m models/ --property-override-list conf/overrides.yml -o gen/

Generate for a specific product variant::

    ingot -m models/ --variant peabodyv0 -o gen/
