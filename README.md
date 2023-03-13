# chatgpt-rs

这是一个使用OpenAI GPT模型的聊天程序，您可以与一个自然语言处理模型互动。试试看吧，您可以与ChatGPT聊天，探讨任何您感兴趣的话题，无论是科技、文化、娱乐或是其他任何主题。

## 如何使用？

1. 在[OpenAI API官网](https://beta.openai.com/)上申请API密钥。

2. 在项目根目录中创建`.env`文件，将申请得到的API密钥放入对应的位置，格式为`OPENAI_API_KEY=YOUR_SECRET_KEY`。

3. 如果您需要使用HTTP代理，则在`.env`文件中添加`HTTP_PROXY=YOUR_PROXY_ADDRESS`配置。

4. 如果您需要在聊天开始之前储存一条系统信息，在 `.env` 文件中添加 `SYSTEM_MESSAGE=YOUR_MESSAGE` 配置。此时第一条聊天记录将以系统身份进行存储。

5. 运行`cargo run`。

## 注意事项

1. 本程序的用途是提供娱乐和交流，不应用于商业目的。请勿利用本程序进行广告、诈骗或其他非法活动。

2. 确保您的API密钥的安全性。请不要将密钥泄露给任何人或组织。

3. 如果您要公开分享该程序，请提醒其他用户注意以上事项。
