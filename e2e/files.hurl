# Files E2E Tests
# Lifecycle: purge -> create -> get -> list -> update -> download -> delete -> verify deleted

# Purge all data
DELETE http://{{host}}:{{port}}/admin/purge
HTTP 200
[Asserts]
jsonpath "$.status" == "success"


# Create a file via multipart upload (with subject_id)
POST http://{{host}}:{{port}}/files
[MultipartFormData]
file: file,sample.txt; text/plain
permalink: docs/sample.txt
name: Sample Document
alt: A sample text document
description: E2E test file for the file-manager service
subject_id: user-e2e-123
metadata: {"source": "e2e-test", "priority": 1}
HTTP 200
[Captures]
file_id: jsonpath "$.data.id"
file_permalink: jsonpath "$.data.permalink"
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.id" isString
jsonpath "$.data.mime_type" == "text/plain"
jsonpath "$.data.file_type" == "document"
jsonpath "$.data.permalink" == "docs/sample.txt"
jsonpath "$.data.name" == "Sample Document"
jsonpath "$.data.alt" == "A sample text document"
jsonpath "$.data.description" == "E2E test file for the file-manager service"
jsonpath "$.data.subject_id" == "user-e2e-123"
jsonpath "$.data.metadata.source" == "e2e-test"
jsonpath "$.data.metadata.priority" == 1
jsonpath "$.data.byte_size" > 0
jsonpath "$.data.created_at" isString
jsonpath "$.data.updated_at" isString


# Get file metadata by ID
GET http://{{host}}:{{port}}/files/{{file_id}}
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.id" == {{file_id}}
jsonpath "$.data.permalink" == "docs/sample.txt"
jsonpath "$.data.name" == "Sample Document"
jsonpath "$.data.subject_id" == "user-e2e-123"
jsonpath "$.data.metadata.source" == "e2e-test"


# List all files (unfiltered)
GET http://{{host}}:{{port}}/files
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.items" count >= 1
jsonpath "$.data.pagination.total" >= 1


# List files filtered by subject_id
GET http://{{host}}:{{port}}/files?subject_id=user-e2e-123
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.items" count >= 1
jsonpath "$.data.items[0].subject_id" == "user-e2e-123"


# List files with non-matching subject_id
GET http://{{host}}:{{port}}/files?subject_id=nonexistent-user
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.items" count == 0
jsonpath "$.data.pagination.total" == 0


# List files filtered by file_type
GET http://{{host}}:{{port}}/files?file_type=document
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.items" count >= 1


# List files with non-matching filter
GET http://{{host}}:{{port}}/files?file_type=video
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.items" count == 0
jsonpath "$.data.pagination.total" == 0


# Update file metadata (including subject_id and metadata)
PUT http://{{host}}:{{port}}/files/{{file_id}}
Content-Type: application/json
{
  "name": "Updated Sample Document",
  "alt": "Updated alt text",
  "description": null,
  "permalink": "docs/updated-sample.txt",
  "subject_id": "org-456",
  "metadata": {"source": "e2e-test", "priority": 2, "reviewed": true}
}
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.name" == "Updated Sample Document"
jsonpath "$.data.alt" == "Updated alt text"
jsonpath "$.data.description" == null
jsonpath "$.data.permalink" == "docs/updated-sample.txt"
jsonpath "$.data.subject_id" == "org-456"
jsonpath "$.data.metadata.priority" == 2
jsonpath "$.data.metadata.reviewed" == true


# Verify subject_id filter returns updated subject
GET http://{{host}}:{{port}}/files?subject_id=org-456
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.items" count >= 1


# Verify old subject_id returns empty
GET http://{{host}}:{{port}}/files?subject_id=user-e2e-123
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.items" count == 0


# Download file content via updated permalink
GET http://{{host}}:{{port}}/static/docs/updated-sample.txt
HTTP 200
[Asserts]
header "Content-Type" == "text/plain"
header "Cache-Control" == "public, max-age=3600"
body contains "sample"


# Verify old permalink returns 404
GET http://{{host}}:{{port}}/static/docs/sample.txt
HTTP 404
[Asserts]
jsonpath "$.status" == "fail"


# Try creating a file with duplicate permalink
POST http://{{host}}:{{port}}/files
[MultipartFormData]
file: file,sample.txt; text/plain
permalink: docs/updated-sample.txt
HTTP 409
[Asserts]
jsonpath "$.status" == "fail"


# Delete file
DELETE http://{{host}}:{{port}}/files/{{file_id}}
HTTP 200
[Asserts]
jsonpath "$.status" == "success"


# Verify deleted file returns 404
GET http://{{host}}:{{port}}/files/{{file_id}}
HTTP 404
[Asserts]
jsonpath "$.status" == "fail"


# Verify download of deleted file returns 404
GET http://{{host}}:{{port}}/static/docs/updated-sample.txt
HTTP 404
[Asserts]
jsonpath "$.status" == "fail"
