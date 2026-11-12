let helper = import "lib/helper.ncl" in
{
  name | String | doc "The server's name" = "my" ++ "server",
  port | Number | default = 80,
  replicas | Number | optional,
  secret | not_exported = "hidden",
  greeting = helper.greet name,
  labels = { app = name, tier = "web" },
}
