INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('AddSubjectTagPrefixSettings', '{
  "$schema": "http://json-schema.org/draft-07/schema",
  "$ref": "#/$defs/AddSubjectTagPrefixSettings",
  "$defs": {
    "AddSubjectTagPrefixSettings": {
      "title": "AddSubjectTagPrefixSettings",
      "description": "Settings for AddSubjectTagPrefix message filter",
      "type": "object",
      "properties": {
        "enabled": {
          "title": "If true, the list subject prefix is added to post subjects.",
          "type": "boolean"
        }
      },
      "required": [
        "enabled"
      ]
    }
  }
}');
