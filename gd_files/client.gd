extends Node

class_name ConvexGd

# The URL we will connect to.
var websocket_url: String
var socket: WebSocketPeer
var convex_client: ConvexClient

func _init(
		_url: String = "",
):
	if _url:
		websocket_url = _url
	
	

func _ready():
	
	if not websocket_url:
		websocket_url = OS.get_environment("CONVEX_URL")
	if websocket_url == "":
		log_message("No CONVEX_URL environment variable set.")
		set_process(false)
		return
	else:
		log_message("Connecting to %s" % websocket_url)
		# create a new websocket peer
		socket = WebSocketPeer.new()
		var res = socket.connect_to_url(websocket_url)
		if res != OK:
			log_message("Unable to connect.")
			set_process(false)
		convex_client = ConvexClient.create()

func _process(_delta):
	# every frame we check if there are any packets to process
	# and we pass them to the convex client
	# the subscribers check on their own if any of these messages are relevant to them
	socket.poll()
	if socket.get_ready_state() == WebSocketPeer.STATE_OPEN:
		# go over all inbound messages and process them
		receive_messages()
		# then flush pending messages
		flush_messages()

func subscribe(udf_path: String, args: Dictionary):
	var res = convex_client.subscribe(udf_path, args)
	if socket.get_ready_state() == WebSocketPeer.STATE_OPEN:
		flush_messages()
	return res

func mutation(udf_path: String, args: Dictionary):
	var res = convex_client.mutation(udf_path, args)
	if socket.get_ready_state() == WebSocketPeer.STATE_OPEN:
		flush_messages()
	return res
	
func action(udf_path: String, args: Dictionary):
	var res = convex_client.action(udf_path, args)
	if socket.get_ready_state() == WebSocketPeer.STATE_OPEN:
		flush_messages()
	return res

func get_results_for_subscription(subscription: Subscription):
	var results = convex_client.get_results_for_subscription(subscription)
	if results and 'data' in results:
		return results['data']
	return null

func log_message(message):
	var time = "[color=#aaaaaa] %s [/color]" % Time.get_time_string_from_system()
	print(message)

func process_packet(pkt):
	# parse the message
	convex_client.receive_message(pkt)


func _exit_tree():
	socket.close()

func send_message(txt):
	var res = socket.send_text(txt)

func receive_messages():
	while socket.get_available_packet_count():
		process_packet(socket.get_packet().get_string_from_ascii())
	

func flush_messages():
	while true:
		var msg = convex_client.pop_next_message()
		if msg:
			send_message(msg)
		else:
			break
