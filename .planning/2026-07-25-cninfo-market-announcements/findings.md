# Findings

- Existing Core `Announcements` and Router `announcement_source` are strictly
  instrument-scoped and validate exact request identity.
- Existing CNInfo instrument announcements first perform organization mapping.
  That path cannot prove whole-market discovery.
- Live official response for `stock=` empty and one fixed date returned
  `totalAnnouncement=1108`, `totalRecordNum=1108`, `totalpages=221`,
  `hasMore=true`, and market rows.
- A Shenzhen plate probe returned 588 records and `pageColumn=SZCY`.
- A Beijing plate probe returned 62 records and `pageColumn=BJS`, including
  code `920189`; this proves exchange must not be inferred from legacy numeric
  prefixes.
- The bounded production probe returned `totalAnnouncement=1108`,
  `pageSize=30`, `totalpages=36`. An earlier page-size-five probe returned
  `totalpages=221`. These values prove CNInfo uses integer quotient
  `total/pageSize`, not ceiling page count; `hasMore` and consumed rows define
  the actual final-page boundary.
- Existing Router unconditionally rejects successful empty batches. A
  default-off policy extension is required to preserve verified empty.
- New focused modules minimize overlap with concurrent dirty Core, Router and
  CNInfo files.
