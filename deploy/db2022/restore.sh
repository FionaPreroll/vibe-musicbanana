#!/bin/bash
# Restores the 2022 musicbanana backup into db2022. The postgres image runs this
# on the container's first start only, while its data volume is still empty.
#
# Everything in /backup is restored: plain SQL (.sql, .sql.gz) through psql,
# anything else (pg_dump -Fc custom format, -Fd directory, -Ft tar) through
# pg_restore. Errors about roles or owners this server does not have are
# printed but do not stop the restore.
set -euo pipefail
shopt -s nullglob

backups=(/backup/*)
if [ ${#backups[@]} -eq 0 ]; then
    echo "db2022: no backup in /backup, starting with an empty database" >&2
    exit 0
fi

psql=(psql --no-psqlrc --quiet --username "$POSTGRES_USER" --dbname "$POSTGRES_DB")
for backup in "${backups[@]}"; do
    echo "db2022: restoring $backup"
    case "$backup" in
        *.sql) "${psql[@]}" --file "$backup" ;;
        *.sql.gz) gunzip --stdout "$backup" | "${psql[@]}" ;;
        *) pg_restore --no-owner --no-privileges --username "$POSTGRES_USER" \
               --dbname "$POSTGRES_DB" "$backup" ||
               echo "db2022: pg_restore reported errors, see above" >&2 ;;
    esac
done
echo "db2022: restore done"
