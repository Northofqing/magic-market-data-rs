use crate::{OfficialNewsError, OfficialSource, MAX_BODY_BYTES};
use magic_market_transport::{EndpointPolicy, MediaType};
use regex::Regex;
use std::time::Duration;
use url::Url;

pub(crate) struct Profile {
    pub api: Option<&'static str>,
    pub listing: &'static str,
    pub prefix: &'static str,
    pub article_path: &'static str,
    pub rows: &'static str,
    pub anchor: &'static str,
    pub date_on_parent: bool,
    pub body: &'static str,
    pub title: &'static str,
}

pub(crate) fn profile(source: OfficialSource) -> Profile {
    match source {
        OfficialSource::Nbs => Profile { api:None, listing:"https://www.stats.gov.cn/sj/zxfb/", prefix:"/sj/zxfb/", article_path:r"^/sj/zxfb/\d{6}/t\d{8}_\d+\.html$", rows:".list-content > ul > li", anchor:"a.pc_1600", date_on_parent:false, body:".detail-text-content .TRS_UEDITOR", title:"" },
        OfficialSource::Pbc => Profile { api:None, listing:"https://www.pbc.gov.cn/goutongjiaoliu/113456/113469/index.html", prefix:"/goutongjiaoliu/113456/113469/", article_path:r"^/goutongjiaoliu/113456/113469/\d+/index\.html$", rows:"font.newslist_style", anchor:"a", date_on_parent:true, body:"#zoom", title:"" },
        OfficialSource::Ndrc => Profile { api:None, listing:"https://www.ndrc.gov.cn/xwdt/xwfb/", prefix:"/xwdt/xwfb/", article_path:r"^/xwdt/xwfb/\d{6}/t\d{8}_\d+\.html$", rows:".u-list > li", anchor:"a", date_on_parent:false, body:".article_con > .TRS_Editor", title:"" },
        OfficialSource::Mof => Profile { api:None, listing:"https://zhs.mof.gov.cn/zhengcefabu/", prefix:"/zhengcefabu/", article_path:r"^/zhengcefabu/\d{6}/t\d{8}_\d+\.htm$", rows:"ul.liBox > li", anchor:"a", date_on_parent:false, body:".my_doccontent > .TRS_Editor", title:"h2.title_con" },
        OfficialSource::Miit => Profile { api:Some("https://www.miit.gov.cn/api-gateway/jpaas-publish-server/front/page/build/unit?parseType=buildstatic&webId=8d828e408d90447786ddbe128d495e9e&tplSetId=209741b2109044b5b7695700b2bec37e&pageType=column&tagId=%E5%8F%B3%E4%BE%A7%E5%86%85%E5%AE%B9&editType=null&pageId=d3e2bede1bc045e2875fc7161c01db7d"), listing:"https://www.miit.gov.cn/xwfb/bldhd/index.html", prefix:"/xwfb/bldhd/", article_path:r"^/xwfb/bldhd/art/\d{4}/art_[a-z0-9]{32}\.html$", rows:"div.page-content > ul > li", anchor:"a.fl", date_on_parent:false, body:"div#con_con.ccontent", title:"h1#con_title" },
        OfficialSource::Mofcom => Profile { api:Some("https://www.mofcom.gov.cn/api-gateway/jpaas-publish-server/front/page/build/unit?parseType=bulidstatic&webId=8f43c7ad3afc411fb56f281724b73708&tplSetId=52551ea0e2c14bca8c84792f7aa37ead&pageType=column&tagId=%E5%88%86%E9%A1%B5%E5%88%97%E8%A1%A8&editType=null&pageId=95d89972d8aa4fcea511701cd0f212d9"), listing:"https://www.mofcom.gov.cn/xwfb/rcxwfb/index.html", prefix:"/xwfb/rcxwfb/", article_path:r"^/xwfb/rcxwfb/art/\d{4}/art_[a-z0-9]{32}\.html$", rows:".txtList_01 > li", anchor:"a", date_on_parent:false, body:"div.art-con[ergodic=article]", title:"" },
        OfficialSource::Gacc => Profile { api:None, listing:"https://www.customs.gov.cn/", prefix:"/customs/", article_path:r"^/customs/[0-9/]+/index\.html$", rows:"", anchor:"a", date_on_parent:false, body:"", title:"" },
        OfficialSource::Nea => Profile { api:None, listing:"https://www.nea.gov.cn/xwzx/index.htm", prefix:"/", article_path:r"^/\d{8}/[a-z0-9]{32}/c\.html$", rows:".xwzx-page01 .xwzx-yw-right .xwzx-yw-box > ul > li", anchor:"a", date_on_parent:false, body:"#detailContent", title:".article-title > .titles" },
        OfficialSource::Csrc => Profile { api:Some("https://www.csrc.gov.cn/searchList/a1a078ee0bc54721ab6b148884c784a8?_isAgg=true&_isJson=true&_pageSize=18&_template=index&_rangeTimeGte=&_channelName=&page=1"), listing:"https://www.csrc.gov.cn/csrc/c100028/common_xq_list.shtml", prefix:"/csrc/c100028/", article_path:r"^/csrc/(c100028|c106311)/c[0-9]+/content\.shtml$", rows:"json", anchor:"a", date_on_parent:false, body:"div.detail-news", title:".main > .content > h2" },
    }
}

pub(crate) fn api_policy(
    source: OfficialSource,
    timeout: Duration,
) -> Result<EndpointPolicy, OfficialNewsError> {
    let url = Url::parse(profile(source).api.ok_or_else(|| {
        OfficialNewsError::Unsupported("source has no verified list API profile".into())
    })?)
    .map_err(|_| OfficialNewsError::Protocol("list API profile URL is invalid".into()))?;
    let keys = url.query_pairs().map(|(key, _)| key.into_owned()).collect();
    EndpointPolicy::new(
        url.host_str()
            .ok_or_else(|| OfficialNewsError::Protocol("API host is missing".into()))?,
        vec![url.path().to_owned()],
        keys,
        vec![MediaType::Json],
        MAX_BODY_BYTES,
        timeout,
    )
    .map_err(OfficialNewsError::from)
}

pub(crate) fn policy(
    source: OfficialSource,
    timeout: Duration,
) -> Result<EndpointPolicy, OfficialNewsError> {
    let profile = profile(source);
    let url = Url::parse(profile.listing)
        .map_err(|error| OfficialNewsError::InvalidRequest(error.to_string()))?;
    let mut prefixes = vec![profile.prefix.into()];
    if source == OfficialSource::Csrc {
        prefixes.push("/csrc/c106311/".into());
    }
    EndpointPolicy::new(
        url.host_str()
            .ok_or_else(|| OfficialNewsError::InvalidRequest("profile host is missing".into()))?,
        prefixes,
        Vec::new(),
        vec![MediaType::Html],
        MAX_BODY_BYTES,
        timeout,
    )
    .map_err(OfficialNewsError::from)
}

pub(crate) fn validate_article(
    source: OfficialSource,
    value: &str,
) -> Result<(), OfficialNewsError> {
    let url = Url::parse(value)
        .map_err(|_| OfficialNewsError::InvalidRequest("article URL is invalid".into()))?;
    let expected = Url::parse(profile(source).listing)
        .map_err(|_| OfficialNewsError::InvalidRequest("profile URL is invalid".into()))?;
    let path = Regex::new(profile(source).article_path)
        .map_err(|_| OfficialNewsError::Protocol("profile article pattern is invalid".into()))?;
    if url.scheme() != "https"
        || url.host_str() != expected.host_str()
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !path.is_match(url.path())
        || value.contains('%')
        || value.contains('\\')
        || value.contains("/../")
        || value.contains("/./")
    {
        return Err(OfficialNewsError::InvalidRequest(
            "article URL is outside the exact source profile".into(),
        ));
    }
    if profile(source).body.is_empty() {
        return Err(OfficialNewsError::Unsupported(
            "article template has not been verified".into(),
        ));
    }
    Ok(())
}
