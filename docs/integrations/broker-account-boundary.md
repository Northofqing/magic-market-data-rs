# 券商账户数据边界

现金、持仓、委托、成交和可用额度是账户私有状态，不是公共市场数据。本 workspace
不通过浏览器已登录页面、Cookie、网页抓取或公共 Provider 暴露这些数据，也不会增加
指向下游交易项目的 path dependency。

生产接入应由独立的 authenticated broker gateway 承担：

```text
broker SDK / official API
        |
        v
authenticated account gateway
  - account identity
  - session and credential lifecycle
  - cash / positions
  - orders / executions
  - idempotency and reconciliation
        |
        v
trading application
```

## 必须保留的身份和证据

- 券商、账户和资金账户标识；
- 交易所、证券代码和资产类别；
- 委托 ID、券商合同号、成交 ID；
- 源业务时间与本地观察时间；
- 查询范围、分页游标、请求 ID 和响应批次 ID；
- 币种、数量单位、金额单位和可用/冻结状态；
- 登录失效、权限不足、部分分页、重复成交和对账差异的显式错误。

## 与本项目的连接方式

应用层可以同时依赖市场数据 crate 和独立券商 gateway，通过 `InstrumentId` 等稳定
标识做组合；`magic-market-router` 仍保持 Provider-neutral，不依赖券商实现。行情
缓存时间不能作为账户业务时间，市场数据 Provider 的成功也不能证明账户查询成功。

浏览器里已经登录某个网站只代表交互会话存在，不构成稳定 API、再分发许可、字段
完整性或自动化授权。需要账户数据时，应使用券商正式 SDK/API 或获得明确授权的终端
接口。
