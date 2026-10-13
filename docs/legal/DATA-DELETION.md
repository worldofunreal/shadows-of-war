# Data deletion and moderation runbook

This procedure fulfills the deletion and reporting options offered in the game
and in the privacy page. Use the in-game controls first; use the operator path
only for verified edge cases.

## Account deletion

Players can delete their own account from their profile after confirming twice.
For an email request, ask for the account ID. If the player cannot provide it,
use the public profile search and confirm ownership with the creation date,
linked platform, or recent matches before acting.

For a verified request that cannot use self-service, run the operator endpoint
from IONOS over loopback. It reads the database secret from the service
environment; never expose this endpoint publicly:

    ssh ionos 'sudo sh -s' <<'REMOTE'
    . /usr/local/etc/sow/sow.env
    curl -sS -X POST http://127.0.0.1:25585/internal/profile/delete \
      -H "Authorization: Bearer $SOW_DB_SECRET" \
      -H "Content-Type: application/json" \
      -d '{"account_id":"<account_id>"}'
    REMOTE

Confirm the response says `"found": true`.

Account-linked profile and identity records are erased. Non-identifying daily
totals can remain for up to 90 days, and competitive match history can remain
without a link to the deleted account.

Verify the public profile no longer exists:

    ssh ionos 'curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:25585/profiles/<account_id>'

Expect HTTP 404. Reply within 45 days with what was erased and the 90-day note.
If a request is denied, give the reason and offer an appeal by reply.

For a child under 13, suspend first, erase the account, and reply to the parent
or guardian. Do not ask the child for identity documents.

## Player reports

The in-game report control blocks the reported account for that player and
stores the report for one year. Review reports oldest-first; do not rely on
email notifications:

    ssh ionos 'sudo valkey-cli -h 127.0.0.1 ZRANGE sow:reports:index 0 19 WITHSCORES'
    ssh ionos 'sudo valkey-cli -h 127.0.0.1 GET "sow:report:<report_id>"'

Review and take any moderation action under the Terms of Service. A report does
not automatically suspend the reported account. Reports remain stored and
logged even when no notification email is sent.
