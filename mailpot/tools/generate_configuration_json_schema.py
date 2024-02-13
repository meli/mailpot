#!/usr/bin/env python3
"""
Example taken from https://jcristharif.com/msgspec/jsonschema.html
"""
import msgspec
from msgspec import Struct, Meta
from typing import Annotated, Optional

Template = Annotated[
    str,
    Meta(
        pattern=".+[{]msg-id[}].*",
        description="""Template for \
`Archived-At` header value, as described in RFC 5064 "The Archived-At \
Message Header Field". The template receives only one string variable \
with the value of the mailing list post `Message-ID` header.

For example, if:

- the template is `http://www.example.com/mid/{msg-id}`
- the `Message-ID` is `<0C2U00F01DFGCR@mailsj-v3.example.com>`

The full header will be generated as:

`Archived-At: <http://www.example.com/mid/0C2U00F01DFGCR@mailsj-v3.example.com>

Note: Surrounding carets in the `Message-ID` value are not required. If \
you wish to preserve them in the URL, set option `preserve-carets` to \
true.""",
        title="Jinja template for header value",
        examples=[
            "https://www.example.com/{msg-id}",
            "https://www.example.com/{msg-id}.html",
        ],
    ),
]

PreserveCarets = Annotated[
    bool, Meta(title="Preserve carets of `Message-ID` in generated value")
]


class ArchivedAtLinkSettings(Struct):
    """Settings for ArchivedAtLink message filter"""

    template: Template
    preserve_carets: PreserveCarets = False


schema = {"$schema": "http://json-schema.org/draft-07/schema"}
schema.update(msgspec.json.schema(ArchivedAtLinkSettings))
print(msgspec.json.format(msgspec.json.encode(schema)).decode("utf-8"))
