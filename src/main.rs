use core::str::FromStr as _;

use anyhow::Context as _;
use dashmap::DashMap;
use nom::{
    error::{Error, ErrorKind},
    Finish as _,
};
use serenity::{
    async_trait,
    model::{
        channel::Message,
        gateway::Ready,
        id::{ChannelId, GuildId, UserId},
    },
    prelude::*,
    utils::MessageBuilder,
};
use shuttle_runtime::SecretStore;
use tracing::{error, info};

mod parser;

const KOCHIKITE_GUILD_ID: u64 = 1066468273568362496;
const EROGAKI_ROLE_ID: u64 = 1066667753706102824;

struct Bot {
    userdata: DashMap<UserId, UserData>,
    channel_ids: Vec<ChannelId>,
}

struct UserData {
    room_pointer: ChannelId,
    is_erogaki: bool,
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        // if message is from bot, ignore
        if msg.author.bot {
            return;
        }

        // Check if the message is from direct message
        match msg.guild_id {
            Some(_) => {
                guild_message(self, &ctx, &msg).await;
            }
            None => {
                direct_message(self, &ctx, &msg).await;
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

//direct message
async fn direct_message(bot: &Bot, ctx: &Context, msg: &Message) {
    // ユーザーがこっちにきてはいけないに存在しない場合は無視
    if GuildId::from(KOCHIKITE_GUILD_ID)
        .member(&ctx.http, &msg.author.id)
        .await
        .is_err()
    {
        return;
    }

    // get user data
    let mut user = bot.userdata.entry(msg.author.id).or_insert(UserData {
        room_pointer: bot.channel_ids[0],
        is_erogaki: false,
    });
    // update user data
    let user = update_user(ctx, &mut user, &msg.author.id).await.unwrap();

    // if message is not command, forward to the room
    if !msg.content.starts_with('!') {
        if let Err(why) = user.room_pointer.say(&ctx.http, &msg.content).await {
            error!("Error sending message: {:?}", why);
        };
        return;
    }

    // if message is command, handle command
    // handle command
    let split_message = msg.content.split_whitespace().collect::<Vec<&str>>();
    let command_name = &split_message[0][1..]; // 先頭の "!" を削除
    let command_args = &split_message[1..];
    let dm = &msg.channel_id;

    match command_name {
        "channel" => channel(dm, ctx, command_args, &bot.channel_ids, user).await,
        "erocheck" => erocheck(dm, ctx, user.is_erogaki).await,
        "help" | "たすけて" | "助けて" => help(dm, ctx, &msg.author.id).await,
        "ping" => ping(dm, ctx).await,

        // Unknown command
        _ => {
            dm.say(&ctx.http, "しらないコマンドだよ").await.unwrap();
        }
    }
}

async fn guild_message(bot: &Bot, ctx: &Context, msg: &Message) {
    // if message does not contains any command, ignore
    let input_string: String = match msg.content.find('!') {
        Some(i) => msg.content[i + 1..]
            .chars()
            .take(msg.content.chars().count())
            .collect(),
        None => match msg.content.find("まなみ、") {
            Some(i) => msg
                .content
                .chars()
                .skip(i + 4)
                .take(msg.content.chars().count() - (i + 4))
                .collect(),
            None => return,
        },
    };

    // get user data
    let mut user = bot.userdata.entry(msg.author.id).or_insert(UserData {
        room_pointer: bot.channel_ids[0],
        is_erogaki: false,
    });
    // update user data
    update_user(ctx, &mut user, &msg.author.id).await.unwrap();

    // dice command
    match parser::parse_dice(&msg.content[1..]).finish() {
        Ok((_, parsed)) => {
            dice(&msg.channel_id, ctx, parsed).await;
            return;
        }
        Err(Error { code, .. }) => {
            if code == ErrorKind::MapRes {
                msg.channel_id
                    .say(&ctx.http, "数字がおかしいよ")
                    .await
                    .unwrap();
                return;
            }
        }
    };

    // handle other command
    let split_message = input_string.split_whitespace().collect::<Vec<&str>>();
    let command_name = split_message[0].trim();
    let command_args = &split_message[1..];
    let reply_channel = &msg.channel_id;

    match command_name {
        "help" | "たすけて" | "助けて" => help(reply_channel, ctx, &msg.author.id).await,
        "isprime" => isprime(reply_channel, ctx, command_args).await,
        // Unknown command
        _ => {
            if msg.content.starts_with('!') {
                reply_channel
                    .say(&ctx.http, "しらないコマンドだよ")
                    .await
                    .unwrap();
            }
        }
    }
}

async fn update_user<'a>(
    ctx: &Context,
    user: &'a mut UserData,
    author: &UserId,
) -> Result<&'a mut UserData, anyhow::Error> {
    // erogaki role check
    user.is_erogaki = author
        .to_user(&ctx.http)
        .await?
        .has_role(&ctx.http, KOCHIKITE_GUILD_ID, EROGAKI_ROLE_ID)
        .await?;
    Ok(user)
}

// commands

// help command
async fn help(reply: &ChannelId, ctx: &Context, user_id: &UserId) {
    // 文章
    let about_me = "# まなみの自己紹介だよ！\n";
    let about_ghostwrite = "## 代筆機能があるよ！\nまなみは代筆ができるよ！　DMに送ってもらったメッセージを`!channel`で指定されたチャンネルに転送するよ！\n";

    let about_dm = "## まなみはDMでコマンドを受け付けるよ！
```
!channel        代筆先のチャンネルについてだよ
!erocheck       あなたがエロガキかどうかを判定するよ
!help           このヘルプを表示するよ
!ping           pong!
```
";

    let about_guild = "## まなみはグループチャットでコマンドを受け付けるよ！
```
![n]d<m>        m面ダイスをn回振るよ
!help           このヘルプを表示するよ
!isprime <n>    nが素数かどうかを判定するよ
```
";

    let mut content = MessageBuilder::new();
    content.push(about_me).push(about_ghostwrite);

    // ユーザーがこっちにきてはいけないに存在する場合はDMの話もする
    if GuildId::from(KOCHIKITE_GUILD_ID)
        .member(&ctx.http, user_id)
        .await
        .is_ok()
    {
        content.push(about_dm);
    }

    content.push(about_guild);
    let content = content.build();

    reply.say(&ctx.http, content).await.unwrap();
}

// change channel
async fn channel(
    reply: &ChannelId,
    ctx: &Context,
    args: &[&str],
    channels: &[ChannelId],
    user: &mut UserData,
) {
    // 引数なしの場合はチャンネル一覧を表示
    if args.is_empty() {
        let mut res = MessageBuilder::new();
        res.push("今は")
            .channel(user.room_pointer)
            .push("で代筆してるよ\n")
            .push("```チャンネル一覧だよ\n");
        for (i, ch) in channels.iter().enumerate() {
            res.push(format!("{i:>2}\t"))
                .push(ch.name(&ctx.http).await.unwrap())
                .push("\n");
        }
        let res = res.push("```").push("使い方: `!channel <ID>`").build();

        reply.say(&ctx.http, &res).await.unwrap();
        return;
    }

    // それ以外の場合は指定されたチャンネルに切り替え
    let Ok(selector) = args[0].parse::<usize>() else {
        reply.say(&ctx.http, "IDは数字で指定してね").await.unwrap();
        return;
    };
    if selector >= channels.len() {
        reply
            .say(&ctx.http, "しらないチャンネルだよ")
            .await
            .unwrap();
        return;
    }

    let next_pointer = channels[selector];
    user.room_pointer = next_pointer;
    reply
        .say(
            &ctx.http,
            MessageBuilder::new()
                .push("送信先を")
                .channel(next_pointer)
                .push("に設定したよ")
                .build(),
        )
        .await
        .unwrap();
}

// dice command
async fn dice(reply: &ChannelId, ctx: &Context, parsed: parser::Dice) {
    // パース
    let parser::Dice { num, dice, cmp } = parsed;

    // 入力のチェック
    if num > 1000000 {
        reply
            .say(&ctx.http, "そんないっぱい振れないよ")
            .await
            .unwrap();
        return;
    } else if num == 0 {
        reply.say(&ctx.http, "じゃあ振らないよ").await.unwrap();
        return;
    }

    // ダイスロール
    let mut sum = 0;
    let mut vec = vec![];
    for _ in 0..num {
        let r = rand::random::<u64>() % dice + 1;
        vec.push(r.to_string());
        sum += u128::from(r);
    }
    // 結果
    let roll_result = format!("{}D{} -> {}", num, dice, sum);
    // 内訳
    let roll_items = format!(" ({})", vec.join(", "));

    // 比較オプション
    let operation_result = cmp.map(|(operator, operand)| {
        let is_ok = parser::cmp_with_operator(&operator, sum, operand);
        let is_ok = if is_ok { "OK" } else { "NG" };
        format!(" {} {} -> {}", Into::<&str>::into(operator), operand, is_ok)
    });

    // メッセージの生成と送信
    let mut res = MessageBuilder::new();
    res.push(roll_result);
    if 1 < dice && roll_items.len() <= 100 {
        res.push(roll_items);
    }
    if let Some(operation_result) = operation_result {
        res.push(operation_result);
    }
    reply.say(&ctx.http, &res.build()).await.unwrap();
}

// erogaki status check
async fn erocheck(reply: &ChannelId, ctx: &Context, is_erogaki: bool) {
    let content = if is_erogaki {
        "エロガキ！！！！"
    } else {
        "エロガキじゃないよ"
    };
    reply.say(&ctx.http, content).await.unwrap();
}

async fn isprime(reply: &ChannelId, ctx: &Context, command_args: &[&str]) {
    if command_args.len() != 1 {
        reply
            .say(&ctx.http, "使い方: `!isprime <number>`")
            .await
            .unwrap();
        return;
    }

    let Ok(num) = command_args[0].parse::<u64>() else {
        reply.say(&ctx.http, "わかんないよ").await.unwrap();
        return;
    };

    let (is_prime, factor) = match num {
        0 | 1 => (false, vec![]),
        2 => (true, vec![2]),
        _ => {
            let mut num = num;
            let mut factor = vec![];

            while num % 2 == 0 {
                num /= 2;
                factor.push(2);
            }

            let mut i = 3;

            while i * i <= num {
                if num % i == 0 {
                    num /= i;
                    factor.push(i);
                } else {
                    i += 2;
                }
            }

            if num != 1 {
                factor.push(num);
            }

            if factor.len() == 1 {
                (true, factor)
            } else {
                (false, factor)
            }
        }
    };

    let is_prime = format!(
        "{}は{}",
        num,
        if is_prime {
            "素数だよ".to_owned()
        } else if factor.is_empty() {
            "素数じゃないよ。あたりまえでしょ？".to_owned()
        } else {
            format!("素数じゃないよ。素因数は{:?}だよ", factor)
        }
    );
    reply.say(&ctx.http, is_prime).await.unwrap();
}

// ping command
async fn ping(reply: &ChannelId, ctx: &Context) {
    reply.say(&ctx.http, "pong").await.unwrap();
}

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let token = secrets
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES;

    let userdata = DashMap::new();
    let channel_ids = vec![
        secrets.get("FREETALK1_ROOM_ID"),
        secrets.get("FREETALK2_ROOM_ID"),
        secrets.get("MADSISTERS_ROOM_ID"),
        secrets.get("SHYBOYS_ROOM_ID"),
        secrets.get("DEBUG_ROOM_ID"),
    ]
    .into_iter()
    .map(|id| ChannelId::from_str(&id.unwrap()).unwrap())
    .collect();

    let client = Client::builder(&token, intents)
        .event_handler(Bot {
            userdata,
            channel_ids,
        })
        .await
        .expect("Err creating client");

    Ok(client.into())
}
