import { query, mutation} from "./_generated/server";
import { v } from "convex/values";

export const get = query({
    args: {},
    handler: async (ctx) => {
      return await ctx.db.query("messages").collect();
    },
  });

// Create a new task with the given text
export const createMessage = mutation({
    args: {
        author: v.string(),
        text: v.string()
    },
    handler: async (ctx, args) => {
      const newMessageId = await ctx.db.insert("messages", { author: args.author, text: args.text });
      return newMessageId;
    },
  });