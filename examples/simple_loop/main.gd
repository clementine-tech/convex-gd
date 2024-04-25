extends Node2D

var client: ConvexGd
var server_url: String

var subscription: Subscription

var sub_val

var checkpoint: int = 1000

var pending_results = []

# Called when the node enters the scene tree for the first time.
func _ready():
	# load env vars
	_load_env_vars()
	# instantiate web socket node
	server_url = OS.get_environment("CONVEX_URL")
	client = ConvexGd.new(server_url)
	add_child(client)
	
	# and subscribe to a service
	subscription = client.subscribe("chat_app:get", {})
	print('subscribed')

# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	var new_val = client.get_results_for_subscription(subscription)
	if new_val != sub_val:
		print("New val!")
		print(new_val)
		print("Hi, {ts}ms elapsed!".format({"ts"=Time.get_ticks_msec()}))
		sub_val = new_val
	var ts = Time.get_ticks_msec()
	if ts > checkpoint:
		checkpoint = checkpoint * 2
		# mutate
		var pending_result = client.mutation(
			"chat_app:createMessage",
			{
				"author": "Godot",
				"text": "Hi, {ts}ms elapsed!".format({"ts"=ts})
			}
		)
		pending_results.append(pending_result)
	# and now loop over pending results and print the output if there is one
	var updated_pending_results = []
	for pr in pending_results:
		var res = pr.get_result()
		if res:
			print("Res, {ts}ms elapsed!".format({"ts"=Time.get_ticks_msec()}))
			print(res)
		else:
			updated_pending_results.append(pr)
	pending_results = updated_pending_results
		

func _load_env_vars():
	# hacky way to load local env variable files
	var file = FileAccess.open("res://.env.local", FileAccess.READ)
	if FileAccess.get_open_error() == OK:
		while file.get_position() < file.get_length():
			var line = file.get_line()
			if line != "" and not line.begins_with("#"):  # Ignore empty lines and comments
				var parts = line.split("=")
				if parts.size() >= 2:
					var key = parts[0]
					var value = "=".join(parts.slice(1, parts.size())) # In case value contains '='
					OS.set_environment(key, value)
		file.close()
	else:
		print("[ERROR] [logger] Failed to open .env file")
