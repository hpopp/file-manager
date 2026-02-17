# Cluster E2E Tests
# Health check and cluster status

# Health check
GET http://{{host}}:{{port}}/_internal/health
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.status" == "ok"
jsonpath "$.data.version" isString


# Cluster status
GET http://{{host}}:{{port}}/_internal/cluster/status
HTTP 200
[Asserts]
jsonpath "$.status" == "success"
jsonpath "$.data.cluster_info.node_id" isString
jsonpath "$.data.cluster_info.role" isString
