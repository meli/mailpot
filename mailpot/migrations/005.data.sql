INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('ArchivedAtLinkSettings', '{
  "$schema": "http://json-schema.org/draft-07/schema",
  "$ref": "#/$defs/ArchivedAtLinkSettings",
  "$defs": {
    "ArchivedAtLinkSettings": {
      "title": "ArchivedAtLinkSettings",
      "description": "Settings for ArchivedAtLink message filter",
      "type": "object",
      "properties": {
        "template": {
          "title": "Jinja template for header value",
          "description": "Template for\n        `Archived-At` header value, as described in RFC 5064 \"The Archived-At\n        Message Header Field\". The template receives only one string variable\n        with the value of the mailing list post `Message-ID` header.\n\n        For example, if:\n\n        - the template is `http://www.example.com/mid/{{msg_id}}`\n        - the `Message-ID` is `<0C2U00F01DFGCR@mailsj-v3.example.com>`\n\n        The full header will be generated as:\n\n        `Archived-At: <http://www.example.com/mid/0C2U00F01DFGCR@mailsj-v3.example.com>\n\n        Note: Surrounding carets in the `Message-ID` value are not required. If\n        you wish to preserve them in the URL, set option `preserve-carets` to\n        true.\n        ",
          "examples": [
            "https://www.example.com/{{msg_id}}",
            "https://www.example.com/{{msg_id}}.html"
          ],
          "type": "string",
          "pattern": ".+[{][{]msg_id[}][}].*"
        },
        "preserve_carets": {
          "title": "Preserve carets of `Message-ID` in generated value",
          "type": "boolean",
          "default": false
        }
      },
      "required": [
        "template"
      ]
    }
  }
}');
